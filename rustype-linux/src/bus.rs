//! The mini message bus: connections, unique and well-known names, match rules, routing.
//! Everything the IBus clients need from dbus-daemon, in one thread, with per-connection
//! output queues that are never written from more than the poll loop.

use crate::dbus::{
    Message, Reader, Writer, ERROR, FLAG_NO_REPLY_EXPECTED, METHOD_CALL, METHOD_RETURN, SIGNAL,
};
use crate::sasl::Sasl;
use crate::wire::{body_bool, body_str, body_u32, encode, Frame, Hdr, MAX_FRAME};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::os::unix::net::UnixStream;

pub const BUS_NAME: &str = "org.freedesktop.DBus";
pub const BUS_PATH: &str = "/org/freedesktop/DBus";
/// Name the server answers for itself. It is also what GetNameOwner returns for these names, and
/// the sender of every signal and internal call, as ibus-daemon does.
pub const IBUS_NAME: &str = "org.freedesktop.IBus";
pub const RUSTYPE_NAME: &str = "org.freedesktop.IBus.Rustype";
pub const PANEL_NAME: &str = "org.freedesktop.IBus.Panel";
pub const PANEL_PATH: &str = "/org/freedesktop/IBus/Panel";
pub const PANEL_IFACE: &str = "org.freedesktop.IBus.Panel";

const NAME_ALLOW_REPLACEMENT: u32 = 0x1;
const NAME_REPLACE_EXISTING: u32 = 0x2;
const NAME_DO_NOT_QUEUE: u32 = 0x4;
const REPLY_PRIMARY_OWNER: u32 = 1;
const REPLY_IN_QUEUE: u32 = 2;
const REPLY_EXISTS: u32 = 3;
const REPLY_ALREADY_OWNER: u32 = 4;
const RELEASE_RELEASED: u32 = 1;
const RELEASE_NON_EXISTENT: u32 = 2;
const RELEASE_NOT_OWNER: u32 = 3;

pub type Fail = (&'static str, String);
pub type Res = Result<(&'static str, Vec<u8>), Fail>;

fn arg_err(e: io::Error) -> Fail {
    ("org.freedesktop.DBus.Error.InvalidArgs", e.to_string())
}

/// Who owns a name: the server itself, or a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Internal,
    Conn(usize),
}

struct NameEntry {
    owner: Owner,
    flags: u32,
    queue: VecDeque<(Owner, u32)>,
}

/// A match rule, kept as the key/value pairs of its string form.
#[derive(Clone, Debug)]
pub struct Rule {
    raw: String,
    kv: Vec<(String, String)>,
}

impl Rule {
    pub fn parse(s: &str) -> Option<Rule> {
        let mut kv = Vec::new();
        for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = part.split_once('=')?;
            kv.push((
                k.trim().to_string(),
                v.trim().trim_matches('\'').to_string(),
            ));
        }
        Some(Rule {
            raw: s.to_string(),
            kv,
        })
    }
}

/// What a broadcast signal looks like to the match rules.
pub struct SigInfo {
    pub sender: String,
    pub path: String,
    pub iface: String,
    pub member: String,
    pub arg0: Option<String>,
}

pub struct Peer {
    pub stream: UnixStream,
    pub rbuf: Vec<u8>,
    pub sasl: Sasl,
    pub begun: bool,
    pub wq: Vec<u8>,
    pub woff: usize,
    pub unique: Option<String>,
    pub uid: u32,
    pub pid: i32,
    pub rules: Vec<Rule>,
    pub dead: bool,
    /// Input processing stopped with a complete message waiting, because the output queue is large.
    pub paused: bool,
}

impl Peer {
    pub fn new(stream: UnixStream, sasl: Sasl, uid: u32, pid: i32) -> Peer {
        Peer {
            stream,
            rbuf: Vec::new(),
            sasl,
            begun: false,
            wq: Vec::new(),
            woff: 0,
            unique: None,
            uid,
            pid,
            rules: Vec::new(),
            dead: false,
            paused: false,
        }
    }

    pub fn pending(&self) -> usize {
        self.wq.len() - self.woff
    }
}

pub struct Bus {
    pub peers: BTreeMap<usize, Peer>,
    names: BTreeMap<String, NameEntry>,
    next_id: usize,
    serial: u32,
    pub uuid: String,
}

fn first_string(sig: &str, body: &[u8]) -> Option<String> {
    if sig.starts_with('s') || sig.starts_with('o') {
        Reader::new(body, false).string().ok()
    } else {
        None
    }
}

impl Bus {
    pub fn new(uuid: String) -> Bus {
        let mut names = BTreeMap::new();
        for n in [IBUS_NAME, RUSTYPE_NAME] {
            names.insert(
                n.to_string(),
                NameEntry {
                    owner: Owner::Internal,
                    flags: 0,
                    queue: VecDeque::new(),
                },
            );
        }
        Bus {
            peers: BTreeMap::new(),
            names,
            next_id: 0,
            serial: 0,
            uuid,
        }
    }

    pub fn add_peer(&mut self, peer: Peer) -> usize {
        self.next_id += 1;
        self.peers.insert(self.next_id, peer);
        self.next_id
    }

    fn next_serial(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1).max(1);
        self.serial
    }

    /// The connection id behind a unique name (":1.N").
    pub fn unique_conn(&self, unique: &str) -> Option<usize> {
        let n: usize = unique.strip_prefix(":1.")?.parse().ok()?;
        self.peers
            .get(&n)
            .filter(|p| p.unique.as_deref() == Some(unique))
            .map(|_| n)
    }

    pub fn resolve(&self, name: &str) -> Option<Owner> {
        if name.starts_with(':') {
            self.unique_conn(name).map(Owner::Conn)
        } else {
            self.names.get(name).map(|e| e.owner)
        }
    }

    /// The string a name resolves to: the unique name, or the server's own name.
    pub fn owner_str(&self, o: Owner) -> String {
        match o {
            Owner::Internal => IBUS_NAME.to_string(),
            Owner::Conn(id) => self
                .peers
                .get(&id)
                .and_then(|p| p.unique.clone())
                .unwrap_or_default(),
        }
    }

    /// Appends bytes to a connection's output queue. A connection whose queue would pass
    /// MAX_FRAME is marked dead and dropped by the server.
    pub fn queue(&mut self, id: usize, bytes: &[u8]) {
        if let Some(p) = self.peers.get_mut(&id) {
            if p.pending() + bytes.len() > MAX_FRAME {
                p.dead = true;
                return;
            }
            p.wq.extend_from_slice(bytes);
        }
    }

    pub fn queue_unique(&mut self, unique: &str, bytes: &[u8]) {
        if let Some(id) = self.unique_conn(unique) {
            self.queue(id, bytes);
        }
    }

    /// Hands a message to whoever owns `name`. Returns false when no connection owns it.
    pub fn queue_name(&mut self, name: &str, bytes: &[u8]) -> bool {
        match self.resolve(name) {
            Some(Owner::Conn(id)) => {
                self.queue(id, bytes);
                true
            }
            _ => false,
        }
    }

    /// Forwards a client's message to another connection, with the bus-set sender.
    pub fn forward(&mut self, target: usize, f: &Frame) {
        let bytes = f.bytes();
        self.queue(target, &bytes);
    }

    // ------------------------------------------------------------ message builders

    /// A signal from the server (sender org.freedesktop.IBus), unicast when `dest` is set.
    pub fn ibus_signal(
        &mut self,
        dest: &str,
        path: &str,
        iface: &str,
        member: &str,
        sig: &str,
        body: &[u8],
    ) -> Vec<u8> {
        let serial = self.next_serial();
        encode(
            SIGNAL,
            0,
            serial,
            Hdr {
                path: Some(path),
                iface: Some(iface),
                member: Some(member),
                dest: Some(dest),
                sender: Some(IBUS_NAME),
                ..Default::default()
            },
            sig,
            body,
        )
    }

    /// A method call from the server to the owner of `name`. No reply is wanted, so the panel
    /// is never blocked on us. Silently dropped when nobody owns the name.
    pub fn call_name(
        &mut self,
        name: &str,
        path: &str,
        iface: &str,
        member: &str,
        sig: &str,
        body: &[u8],
    ) {
        if self.resolve(name).is_none() {
            return;
        }
        let serial = self.next_serial();
        let bytes = encode(
            METHOD_CALL,
            FLAG_NO_REPLY_EXPECTED,
            serial,
            Hdr {
                path: Some(path),
                iface: Some(iface),
                member: Some(member),
                dest: Some(name),
                sender: Some(IBUS_NAME),
                ..Default::default()
            },
            sig,
            body,
        );
        self.queue_name(name, &bytes);
    }

    pub fn reply_to(&mut self, from: &str, dest: &str, reply_serial: u32, sig: &str, body: &[u8]) {
        let serial = self.next_serial();
        let bytes = encode(
            METHOD_RETURN,
            0,
            serial,
            Hdr {
                reply: Some(reply_serial),
                dest: Some(dest),
                sender: Some(from),
                ..Default::default()
            },
            sig,
            body,
        );
        self.queue_unique(dest, &bytes);
    }

    /// Replies to `call` (nothing when the caller asked for no reply).
    pub fn reply(&mut self, from: &str, call: &Message, sig: &str, body: &[u8]) {
        if !call.wants_reply() {
            return;
        }
        let dest = call.sender.clone().unwrap_or_default();
        self.reply_to(from, &dest, call.serial, sig, body);
    }

    pub fn error(&mut self, from: &str, call: &Message, name: &str, text: &str) {
        if !call.wants_reply() {
            return;
        }
        let serial = self.next_serial();
        let dest = call.sender.clone().unwrap_or_default();
        let bytes = encode(
            ERROR,
            0,
            serial,
            Hdr {
                error: Some(name),
                reply: Some(call.serial),
                dest: Some(&dest),
                sender: Some(from),
                ..Default::default()
            },
            "s",
            &body_str(text),
        );
        self.queue_unique(&dest, &bytes);
    }

    // ------------------------------------------------------------ broadcast

    /// Broadcast signal from the bus or the server: every connection with a matching rule.
    pub fn broadcast_signal(
        &mut self,
        sender: &str,
        path: &str,
        iface: &str,
        member: &str,
        sig: &str,
        body: &[u8],
    ) {
        let serial = self.next_serial();
        let bytes = encode(
            SIGNAL,
            0,
            serial,
            Hdr {
                path: Some(path),
                iface: Some(iface),
                member: Some(member),
                sender: Some(sender),
                ..Default::default()
            },
            sig,
            body,
        );
        let info = SigInfo {
            sender: sender.to_string(),
            path: path.to_string(),
            iface: iface.to_string(),
            member: member.to_string(),
            arg0: first_string(sig, body),
        };
        self.broadcast(&bytes, &info);
    }

    /// Broadcasts a client's undirected signal to the connections whose rules match it.
    pub fn broadcast_frame(&mut self, f: &Frame) {
        let m = &f.msg;
        let info = SigInfo {
            sender: m.sender.clone().unwrap_or_default(),
            path: m.path.clone().unwrap_or_default(),
            iface: m.interface.clone().unwrap_or_default(),
            member: m.member.clone().unwrap_or_default(),
            arg0: first_string(&m.signature, &m.body),
        };
        let bytes = f.bytes();
        self.broadcast(&bytes, &info);
    }

    fn broadcast(&mut self, bytes: &[u8], info: &SigInfo) {
        let ids: Vec<usize> = self
            .peers
            .iter()
            .filter(|(_, p)| {
                p.unique.is_some() && p.rules.iter().any(|r| self.rule_matches(r, info))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.queue(id, bytes);
        }
    }

    fn rule_matches(&self, rule: &Rule, info: &SigInfo) -> bool {
        rule.kv.iter().all(|(k, v)| match k.as_str() {
            "type" => v == "signal",
            "sender" => {
                *v == info.sender
                    || self
                        .resolve(v)
                        .map(|o| self.owner_str(o))
                        .is_some_and(|s| s == info.sender)
            }
            "interface" => *v == info.iface,
            "member" => *v == info.member,
            "path" => *v == info.path,
            "path_namespace" => {
                let ns = v.trim_end_matches('/');
                info.path == *v || info.path.starts_with(&format!("{ns}/"))
            }
            "arg0" => info.arg0.as_deref() == Some(v.as_str()),
            _ => true,
        })
    }

    fn owner_changed(&mut self, name: &str, old: &str, new: &str) {
        let mut w = Writer::new();
        w.string(name);
        w.string(old);
        w.string(new);
        let body = w.buf;
        self.broadcast_signal(
            BUS_NAME,
            BUS_PATH,
            BUS_NAME,
            "NameOwnerChanged",
            "sss",
            &body,
        );
    }

    fn name_signal(&mut self, to: &str, member: &str, name: &str) {
        let body = body_str(name);
        let serial = self.next_serial();
        let bytes = encode(
            SIGNAL,
            0,
            serial,
            Hdr {
                path: Some(BUS_PATH),
                iface: Some(BUS_NAME),
                member: Some(member),
                dest: Some(to),
                sender: Some(BUS_NAME),
                ..Default::default()
            },
            "s",
            &body,
        );
        self.queue_unique(to, &bytes);
    }

    // ------------------------------------------------------------ names

    fn unique_of(&self, id: usize) -> String {
        self.peers
            .get(&id)
            .and_then(|p| p.unique.clone())
            .unwrap_or_default()
    }

    /// Hands `name` to the next queued owner, or frees it. `old` is the owner's unique name.
    fn give_up(&mut self, name: &str, old: String, notify_lost: bool) {
        let Some(entry) = self.names.remove(name) else {
            return;
        };
        if notify_lost {
            self.name_signal(&old, "NameLost", name);
        }
        let mut queue = entry.queue;
        match queue.pop_front() {
            Some((next, flags)) => {
                let new = self.owner_str(next);
                self.names.insert(
                    name.to_string(),
                    NameEntry {
                        owner: next,
                        flags,
                        queue,
                    },
                );
                self.owner_changed(name, &old, &new);
                self.name_signal(&new, "NameAcquired", name);
            }
            None => self.owner_changed(name, &old, ""),
        }
    }

    /// RequestName with dbus-daemon's rules: free name -> primary owner; already ours -> 4;
    /// REPLACE_EXISTING against an owner that allows replacement -> takes it over (the old owner
    /// is queued again unless it set DO_NOT_QUEUE); DO_NOT_QUEUE -> exists; otherwise queued.
    pub fn request_name(&mut self, id: usize, name: &str, flags: u32) -> u32 {
        let me = Owner::Conn(id);
        let my = self.unique_of(id);
        let existing = self
            .names
            .get(name)
            .map(|e| (e.owner, e.flags, e.queue.iter().any(|(o, _)| *o == me)));
        let Some((owner, oflags, queued)) = existing else {
            self.names.insert(
                name.to_string(),
                NameEntry {
                    owner: me,
                    flags,
                    queue: VecDeque::new(),
                },
            );
            self.owner_changed(name, "", &my);
            self.name_signal(&my, "NameAcquired", name);
            return REPLY_PRIMARY_OWNER;
        };
        if owner == me {
            return REPLY_ALREADY_OWNER;
        }
        if flags & NAME_REPLACE_EXISTING != 0 && oflags & NAME_ALLOW_REPLACEMENT != 0 {
            let old = self.owner_str(owner);
            let mut queue = self.names.remove(name).map(|e| e.queue).unwrap_or_default();
            if oflags & NAME_DO_NOT_QUEUE == 0 {
                queue.push_front((owner, oflags));
            }
            self.names.insert(
                name.to_string(),
                NameEntry {
                    owner: me,
                    flags,
                    queue,
                },
            );
            if let Owner::Conn(_) = owner {
                self.name_signal(&old, "NameLost", name);
            }
            self.owner_changed(name, &old, &my);
            self.name_signal(&my, "NameAcquired", name);
            return REPLY_PRIMARY_OWNER;
        }
        if flags & NAME_DO_NOT_QUEUE != 0 {
            return REPLY_EXISTS;
        }
        if !queued {
            if let Some(e) = self.names.get_mut(name) {
                e.queue.push_back((me, flags));
            }
        }
        REPLY_IN_QUEUE
    }

    pub fn release_name(&mut self, id: usize, name: &str) -> u32 {
        let me = Owner::Conn(id);
        let Some(owner) = self.names.get(name).map(|e| e.owner) else {
            return RELEASE_NON_EXISTENT;
        };
        if owner == me {
            let my = self.unique_of(id);
            self.give_up(name, my, true);
            return RELEASE_RELEASED;
        }
        let Some(e) = self.names.get_mut(name) else {
            return RELEASE_NON_EXISTENT;
        };
        let before = e.queue.len();
        e.queue.retain(|(o, _)| *o != me);
        if e.queue.len() != before {
            RELEASE_RELEASED
        } else {
            RELEASE_NOT_OWNER
        }
    }

    /// Drops a connection: frees or hands on its names, and removes it from every queue.
    /// Returns its unique name when it had one.
    pub fn remove_peer(&mut self, id: usize) -> Option<String> {
        let peer = self.peers.remove(&id)?;
        let unique = peer.unique?;
        let me = Owner::Conn(id);
        let names: Vec<String> = self.names.keys().cloned().collect();
        for name in names {
            let Some(e) = self.names.get_mut(&name) else {
                continue;
            };
            e.queue.retain(|(o, _)| *o != me);
            if e.owner == me {
                self.give_up(&name, unique.clone(), false);
            }
        }
        Some(unique)
    }

    /// Hello: assigns the unique name (":1.<id>"), then NameAcquired for it.
    pub fn hello(&mut self, id: usize, call: &Message) {
        let unique = format!(":1.{id}");
        if let Some(p) = self.peers.get_mut(&id) {
            p.unique = Some(unique.clone());
        }
        self.reply_to(BUS_NAME, &unique, call.serial, "s", &body_str(&unique));
        self.owner_changed(&unique, "", &unique);
        self.name_signal(&unique, "NameAcquired", &unique);
    }

    // ------------------------------------------------------------ org.freedesktop.DBus

    /// Calls addressed to org.freedesktop.DBus (or to the bus path).
    pub fn bus_call(&mut self, id: usize, m: &Message) {
        let iface = m.interface.clone().unwrap_or_default();
        let member = m.member.clone().unwrap_or_default();
        match self.bus_method(id, m, &iface, &member) {
            Ok((sig, body)) => self.reply(BUS_NAME, m, sig, &body),
            Err((name, text)) => self.error(BUS_NAME, m, name, &text),
        }
    }

    fn bus_method(&mut self, id: usize, m: &Message, iface: &str, member: &str) -> Res {
        let mut r = m.body_reader();
        match (iface, member) {
            ("org.freedesktop.DBus.Peer", "Ping") => Ok(("", Vec::new())),
            ("org.freedesktop.DBus.Peer", "GetMachineId") => Ok(("s", body_str(&self.uuid))),
            ("org.freedesktop.DBus", "Hello") => Err((
                "org.freedesktop.DBus.Error.Failed",
                "Already handled Hello".into(),
            )),
            ("org.freedesktop.DBus", "RequestName") => {
                let name = r.string().map_err(arg_err)?;
                let flags = r.u32().map_err(arg_err)?;
                if name.is_empty() || name.starts_with(':') || name == BUS_NAME {
                    return Err((
                        "org.freedesktop.DBus.Error.InvalidArgs",
                        format!("invalid name {name}"),
                    ));
                }
                Ok(("u", body_u32(self.request_name(id, &name, flags))))
            }
            ("org.freedesktop.DBus", "ReleaseName") => {
                let name = r.string().map_err(arg_err)?;
                Ok(("u", body_u32(self.release_name(id, &name))))
            }
            ("org.freedesktop.DBus", "NameHasOwner") => {
                let name = r.string().map_err(arg_err)?;
                let has = name == BUS_NAME || self.resolve(&name).is_some();
                Ok(("b", body_bool(has)))
            }
            ("org.freedesktop.DBus", "GetNameOwner") => {
                let name = r.string().map_err(arg_err)?;
                if name == BUS_NAME {
                    return Ok(("s", body_str(BUS_NAME)));
                }
                match self.resolve(&name) {
                    Some(o) => Ok(("s", body_str(&self.owner_str(o)))),
                    None => Err((
                        "org.freedesktop.DBus.Error.NameHasNoOwner",
                        format!("Could not get owner of name '{name}': no such name"),
                    )),
                }
            }
            ("org.freedesktop.DBus", "ListNames") => {
                let mut all = vec![BUS_NAME.to_string()];
                all.extend(self.peers.values().filter_map(|p| p.unique.clone()));
                all.extend(self.names.keys().cloned());
                Ok(("as", string_array(&all)))
            }
            ("org.freedesktop.DBus", "ListQueuedOwners") => {
                let name = r.string().map_err(arg_err)?;
                let Some(e) = self.names.get(&name) else {
                    return Err((
                        "org.freedesktop.DBus.Error.NameHasNoOwner",
                        format!("Could not get owners of name '{name}': no such name"),
                    ));
                };
                let mut owners = vec![self.owner_str(e.owner)];
                let queued: Vec<Owner> = e.queue.iter().map(|(o, _)| *o).collect();
                owners.extend(queued.into_iter().map(|o| self.owner_str(o)));
                Ok(("as", string_array(&owners)))
            }
            ("org.freedesktop.DBus", "AddMatch") => {
                let text = r.string().map_err(arg_err)?;
                let rule = Rule::parse(&text)
                    .ok_or(("org.freedesktop.DBus.Error.MatchRuleInvalid", text.clone()))?;
                if let Some(p) = self.peers.get_mut(&id) {
                    p.rules.push(rule);
                }
                Ok(("", Vec::new()))
            }
            ("org.freedesktop.DBus", "RemoveMatch") => {
                let text = r.string().map_err(arg_err)?;
                let removed = self.peers.get_mut(&id).is_some_and(|p| {
                    match p.rules.iter().position(|x| x.raw == text) {
                        Some(i) => {
                            p.rules.remove(i);
                            true
                        }
                        None => false,
                    }
                });
                if removed {
                    Ok(("", Vec::new()))
                } else {
                    Err(("org.freedesktop.DBus.Error.MatchRuleNotFound", text))
                }
            }
            ("org.freedesktop.DBus", "GetId") => Ok(("s", body_str(&self.uuid))),
            ("org.freedesktop.DBus", "GetConnectionUnixUser")
            | ("org.freedesktop.DBus", "GetConnectionUnixProcessID") => {
                let name = r.string().map_err(arg_err)?;
                let conn = self.resolve(&name).and_then(|o| match o {
                    Owner::Conn(c) => Some(c),
                    Owner::Internal => None,
                });
                let p = conn
                    .and_then(|c| self.peers.get(&c))
                    .ok_or(("org.freedesktop.DBus.Error.NameHasNoOwner", name.clone()))?;
                let v = if member == "GetConnectionUnixUser" {
                    p.uid
                } else {
                    p.pid as u32
                };
                Ok(("u", body_u32(v)))
            }
            ("org.freedesktop.DBus", "ReloadConfig")
            | ("org.freedesktop.DBus", "UpdateActivationEnvironment") => Ok(("", Vec::new())),
            _ => Err((
                "org.freedesktop.DBus.Error.UnknownMethod",
                format!("No such method {member}"),
            )),
        }
    }
}

/// `as` body for a list of strings.
pub fn string_array(items: &[String]) -> Vec<u8> {
    let mut w = Writer::new();
    let arr = w.array_start(4);
    for s in items {
        w.string(s);
    }
    w.array_end(arr);
    w.buf
}

/// True for frames the bus itself answers (dest org.freedesktop.DBus, or the bus path).
pub fn is_bus_call(f: &Frame) -> bool {
    f.dest.as_deref() == Some(BUS_NAME)
        || (f.dest.is_none() && f.msg.path.as_deref() == Some(BUS_PATH))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::parse;
    use std::os::unix::net::UnixStream;

    /// A connection with a unique name and a socket pair whose other end the test can read.
    fn peer(bus: &mut Bus) -> (usize, UnixStream) {
        let (a, b) = UnixStream::pair().unwrap();
        a.set_nonblocking(true).unwrap();
        let id = bus.add_peer(Peer::new(a, Sasl::new("00", 0, 0), 0, 0));
        bus.peers.get_mut(&id).unwrap().unique = Some(format!(":1.{id}"));
        (id, b)
    }

    /// Takes everything queued for `id` and returns the parsed messages.
    fn drain(bus: &mut Bus, id: usize) -> Vec<Message> {
        let p = bus.peers.get_mut(&id).unwrap();
        let bytes = std::mem::take(&mut p.wq);
        p.woff = 0;
        let mut out = Vec::new();
        let mut rest: &[u8] = &bytes;
        while !rest.is_empty() {
            let n = crate::wire::frame_len(rest).unwrap().unwrap();
            out.push(parse(&rest[..n]).unwrap().msg);
            rest = &rest[n..];
        }
        out
    }

    fn call(
        sender: &str,
        serial: u32,
        _dest: &str,
        member: &str,
        sig: &str,
        body: &[u8],
    ) -> Message {
        Message {
            mtype: METHOD_CALL,
            serial,
            path: Some(BUS_PATH.into()),
            interface: Some(BUS_NAME.into()),
            member: Some(member.into()),
            sender: Some(sender.into()),
            signature: sig.into(),
            body: body.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn request_name_free_then_already_owner_then_queued() {
        let mut bus = Bus::new("uuid".into());
        let (a, _ra) = peer(&mut bus);
        let (b, _rb) = peer(&mut bus);
        assert_eq!(bus.request_name(a, "x.y", 0), REPLY_PRIMARY_OWNER);
        assert_eq!(bus.request_name(a, "x.y", 0), REPLY_ALREADY_OWNER);
        assert_eq!(bus.request_name(b, "x.y", 0), REPLY_IN_QUEUE);
        assert_eq!(bus.request_name(b, "x.y", NAME_DO_NOT_QUEUE), REPLY_EXISTS);
        assert_eq!(
            bus.owner_str(bus.resolve("x.y").unwrap()),
            format!(":1.{a}")
        );
    }

    #[test]
    fn replace_existing_needs_allow_replacement() {
        let mut bus = Bus::new("uuid".into());
        let (a, _ra) = peer(&mut bus);
        let (b, _rb) = peer(&mut bus);
        assert_eq!(bus.request_name(a, "p", 0), REPLY_PRIMARY_OWNER);
        // Owner did not allow replacement: REPLACE_EXISTING is only a queue request.
        assert_eq!(
            bus.request_name(b, "p", NAME_REPLACE_EXISTING),
            REPLY_IN_QUEUE
        );
        assert_eq!(bus.resolve("p"), Some(Owner::Conn(a)));

        let (c, _rc) = peer(&mut bus);
        assert_eq!(
            bus.request_name(a, "q", NAME_ALLOW_REPLACEMENT | NAME_DO_NOT_QUEUE),
            REPLY_PRIMARY_OWNER
        );
        assert_eq!(
            bus.request_name(c, "q", NAME_REPLACE_EXISTING),
            REPLY_PRIMARY_OWNER
        );
        assert_eq!(bus.resolve("q"), Some(Owner::Conn(c)));
    }

    #[test]
    fn release_hands_the_name_to_the_queue_and_disconnect_frees_it() {
        let mut bus = Bus::new("uuid".into());
        let (a, _ra) = peer(&mut bus);
        let (b, _rb) = peer(&mut bus);
        let (c, _rc) = peer(&mut bus);
        bus.request_name(a, "n", 0);
        bus.request_name(b, "n", 0);
        bus.request_name(c, "n", 0);
        assert_eq!(
            bus.release_name(b, "n"),
            RELEASE_RELEASED,
            "queued waiter leaves the queue"
        );
        assert_eq!(bus.release_name(c, "n"), RELEASE_RELEASED);
        assert_eq!(bus.release_name(c, "n"), RELEASE_NOT_OWNER);
        assert_eq!(bus.release_name(a, "missing"), RELEASE_NON_EXISTENT);
        assert_eq!(bus.release_name(a, "n"), RELEASE_RELEASED);
        assert!(bus.resolve("n").is_none());

        // Owner disconnects with a waiter queued: the waiter gets the name.
        bus.request_name(a, "m", 0);
        bus.request_name(b, "m", 0);
        bus.remove_peer(a);
        assert_eq!(bus.resolve("m"), Some(Owner::Conn(b)));
        bus.remove_peer(b);
        assert!(bus.resolve("m").is_none());
    }

    #[test]
    fn owner_changes_are_announced_to_matching_rules_only() {
        let mut bus = Bus::new("uuid".into());
        let (a, _ra) = peer(&mut bus);
        let (watcher, _rw) = peer(&mut bus);
        let (other, _ro) = peer(&mut bus);
        bus.peers.get_mut(&watcher).unwrap().rules.push(
            Rule::parse(
                "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',\
                 member='NameOwnerChanged',arg0='org.freedesktop.IBus.Panel'",
            )
            .unwrap(),
        );
        bus.request_name(a, "org.freedesktop.IBus.Panel", 0);
        let got = drain(&mut bus, watcher);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].member.as_deref(), Some("NameOwnerChanged"));
        assert!(drain(&mut bus, other).is_empty(), "no rule, no signal");
        // The owner itself gets NameAcquired, unicast.
        let own = drain(&mut bus, a);
        assert!(own
            .iter()
            .any(|m| m.member.as_deref() == Some("NameAcquired")));
    }

    #[test]
    fn server_names_resolve_to_the_ibus_name() {
        let bus = Bus::new("uuid".into());
        assert_eq!(bus.resolve(IBUS_NAME), Some(Owner::Internal));
        assert_eq!(bus.owner_str(Owner::Internal), IBUS_NAME);
        assert!(bus.resolve("org.freedesktop.IBus.Panel").is_none());
    }

    #[test]
    fn get_name_owner_and_list_names_reply_shapes() {
        let mut bus = Bus::new("uuid".into());
        let (a, _ra) = peer(&mut bus);
        let m = call(
            ":1.9",
            3,
            BUS_NAME,
            "GetNameOwner",
            "s",
            &body_str(IBUS_NAME),
        );
        let (sig, body) = bus.bus_method(a, &m, BUS_NAME, "GetNameOwner").unwrap();
        assert_eq!(sig, "s");
        assert_eq!(Reader::new(&body, false).string().unwrap(), IBUS_NAME);
        let m = call(
            ":1.9",
            4,
            BUS_NAME,
            "GetNameOwner",
            "s",
            &body_str("nobody.here"),
        );
        let err = bus.bus_method(a, &m, BUS_NAME, "GetNameOwner").unwrap_err();
        assert_eq!(err.0, "org.freedesktop.DBus.Error.NameHasNoOwner");
        // RequestName through the dispatcher: a free name is primary owner (1).
        let mut w = Writer::new();
        w.string("svc.free");
        w.u32(0);
        let m = call(":1.9", 5, BUS_NAME, "RequestName", "su", &w.buf);
        let (sig, body) = bus.bus_method(a, &m, BUS_NAME, "RequestName").unwrap();
        assert_eq!(sig, "u");
        assert_eq!(
            Reader::new(&body, false).u32().unwrap(),
            REPLY_PRIMARY_OWNER
        );
    }

    #[test]
    fn forwarding_keeps_serial_and_sets_sender_and_dest() {
        let mut bus = Bus::new("uuid".into());
        let (a, _ra) = peer(&mut bus);
        let (b, _rb) = peer(&mut bus);
        let bytes = encode(
            METHOD_CALL,
            0,
            77,
            Hdr {
                path: Some("/x"),
                iface: Some("i.f"),
                member: Some("M"),
                dest: Some("svc.name"),
                ..Default::default()
            },
            "",
            &[],
        );
        let mut f = crate::wire::parse(&bytes).unwrap();
        f.msg.sender = Some(format!(":1.{a}"));
        bus.forward(b, &f);
        let got = drain(&mut bus, b);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].serial, 77);
        assert_eq!(got[0].sender.as_deref(), Some(format!(":1.{a}").as_str()));
        assert_eq!(got[0].member.as_deref(), Some("M"));
    }

    #[test]
    fn a_client_whose_queue_passes_16_mib_is_dropped() {
        let mut bus = Bus::new("uuid".into());
        let (a, _ra) = peer(&mut bus);
        let chunk = vec![0u8; 1 << 20];
        for _ in 0..16 {
            bus.queue(a, &chunk);
        }
        assert!(!bus.peers[&a].dead, "exactly 16 MiB is still allowed");
        bus.queue(a, &chunk);
        assert!(
            bus.peers[&a].dead,
            "a queue past 16 MiB marks the client dead"
        );
        assert_eq!(
            bus.peers[&a].pending(),
            16 << 20,
            "the overflowing bytes are not queued"
        );
    }

    #[test]
    fn match_rules_parse_and_remove() {
        let r = Rule::parse("type='signal',interface='org.freedesktop.IBus'").unwrap();
        assert_eq!(r.kv.len(), 2);
        let mut bus = Bus::new("uuid".into());
        let (a, _ra) = peer(&mut bus);
        let m = call("", 1, BUS_NAME, "AddMatch", "s", &body_str("type='signal'"));
        assert!(bus.bus_method(a, &m, BUS_NAME, "AddMatch").is_ok());
        assert!(bus.bus_method(a, &m, BUS_NAME, "RemoveMatch").is_ok());
        assert!(bus.bus_method(a, &m, BUS_NAME, "RemoveMatch").is_err());
    }
}

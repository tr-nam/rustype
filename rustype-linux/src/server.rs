//! The single-threaded poll loop: the listener, every client socket, framing, dispatch to the
//! bus or the IBus service, and per-connection output queues that never block the loop.

use crate::bus::{is_bus_call, Bus, Owner, Peer, PANEL_IFACE, PANEL_NAME, PANEL_PATH};
use crate::sasl::{Event, Sasl};
use crate::service::IBusService;
use crate::sys::{self, PollFd, POLLERR, POLLHUP, POLLIN, POLLOUT};
use crate::wire::{self, Frame};
use rustype_ibus::dbus::{ERROR, METHOD_CALL, METHOD_RETURN, SIGNAL};
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;

/// Upper bound on bytes read from one client per loop turn, so one client cannot starve others.
const READ_BUDGET: usize = 4 << 20;
const POLL_MS: i32 = 200;
/// A client with more than this queued for it is not read from or processed until it drains.
/// Clients that stop reading are still dropped at MAX_FRAME, by Bus::queue.
const INPUT_PAUSE: usize = 1 << 20;

pub struct Server {
    listener: UnixListener,
    guid: String,
    pub bus: Bus,
    pub ibus: IBusService,
}

impl Server {
    pub fn new(listener: UnixListener, guid: String, ibus: IBusService) -> io::Result<Server> {
        listener.set_nonblocking(true)?;
        let uuid = guid.clone();
        Ok(Server {
            listener,
            guid,
            bus: Bus::new(uuid),
            ibus,
        })
    }

    /// Runs until SIGINT/SIGTERM or an IBus Exit call.
    pub fn run(&mut self) -> io::Result<()> {
        let mut fds: Vec<PollFd> = Vec::new();
        loop {
            if sys::stop_requested() || self.ibus.exit_requested {
                return Ok(());
            }
            fds.clear();
            fds.push(PollFd {
                fd: self.listener.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            });
            let ids: Vec<usize> = self.bus.peers.keys().copied().collect();
            for id in &ids {
                let p = &self.bus.peers[id];
                let mut events = 0;
                if p.pending() <= INPUT_PAUSE {
                    events |= POLLIN;
                }
                if p.pending() > 0 {
                    events |= POLLOUT;
                }
                fds.push(PollFd {
                    fd: p.stream.as_raw_fd(),
                    events,
                    revents: 0,
                });
            }
            // Input that was paused and is now allowed to run must not wait for the next socket event.
            let ready_backlog = self
                .bus
                .peers
                .values()
                .any(|p| p.paused && p.pending() <= INPUT_PAUSE);
            sys::poll_fds(&mut fds, if ready_backlog { 0 } else { POLL_MS })?;

            if fds[0].revents & POLLIN != 0 {
                self.accept_all();
            }
            for (i, id) in ids.iter().enumerate() {
                if fds[i + 1].revents & (POLLIN | POLLHUP | POLLERR) != 0 {
                    self.read_peer(*id);
                }
            }
            let ids: Vec<usize> = self.bus.peers.keys().copied().collect();
            for id in &ids {
                self.process(*id);
            }
            for id in &ids {
                self.flush(*id);
            }
            self.reap();
        }
    }

    fn accept_all(&mut self) {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if stream.set_nonblocking(true).is_err() {
                        continue;
                    }
                    let (pid, uid) = sys::peer_cred(stream.as_raw_fd()).unwrap_or((0, u32::MAX));
                    let sasl = Sasl::new(&self.guid, sys::euid(), uid);
                    self.bus.add_peer(Peer::new(stream, sasl, uid, pid));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    }

    fn read_peer(&mut self, id: usize) {
        let Some(p) = self.bus.peers.get_mut(&id) else {
            return;
        };
        if p.pending() > INPUT_PAUSE {
            return;
        }
        let mut buf = [0u8; 64 << 10];
        let mut total = 0;
        while total < READ_BUDGET {
            match p.stream.read(&mut buf) {
                Ok(0) => {
                    p.dead = true;
                    return;
                }
                Ok(n) => {
                    p.rbuf.extend_from_slice(&buf[..n]);
                    total += n;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    p.dead = true;
                    return;
                }
            }
        }
    }

    /// Runs every complete message (or the SASL lines) that the peer has sent so far.
    fn process(&mut self, id: usize) {
        loop {
            let Some(p) = self.bus.peers.get_mut(&id) else {
                return;
            };
            if p.dead {
                return;
            }
            p.paused = false;
            if !p.begun {
                let (out, ev) = p.sasl.feed(&mut p.rbuf);
                p.wq.extend_from_slice(&out);
                match ev {
                    Event::Wait => return,
                    Event::Failed => {
                        p.dead = true;
                        return;
                    }
                    Event::Begun => {
                        p.begun = true;
                        continue;
                    }
                }
            }
            let n = match wire::frame_len(&p.rbuf) {
                Ok(Some(n)) => n,
                Ok(None) => return,
                Err(_) => {
                    p.dead = true;
                    return;
                }
            };
            if p.pending() > INPUT_PAUSE {
                p.paused = true;
                return;
            }
            let bytes: Vec<u8> = p.rbuf.drain(..n).collect();
            let frame = match wire::parse(&bytes) {
                Ok(f) => f,
                Err(_) => {
                    p.dead = true;
                    return;
                }
            };
            self.dispatch(id, frame);
        }
    }

    fn flush(&mut self, id: usize) {
        let Some(p) = self.bus.peers.get_mut(&id) else {
            return;
        };
        while p.woff < p.wq.len() {
            match p.stream.write(&p.wq[p.woff..]) {
                Ok(0) => {
                    p.dead = true;
                    break;
                }
                Ok(n) => p.woff += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    p.dead = true;
                    break;
                }
            }
        }
        if p.woff == p.wq.len() {
            p.wq.clear();
            p.woff = 0;
        } else if p.woff > (1 << 20) {
            p.wq.drain(..p.woff);
            p.woff = 0;
        }
    }

    fn reap(&mut self) {
        let dead: Vec<usize> = self
            .bus
            .peers
            .iter()
            .filter(|(_, p)| p.dead)
            .map(|(id, _)| *id)
            .collect();
        for id in dead {
            if let Some(unique) = self.bus.remove_peer(id) {
                self.ibus.on_peer_gone(&mut self.bus, &unique);
            }
        }
    }

    /// Routes one message from connection `id`. The sender header is always set here.
    fn dispatch(&mut self, id: usize, mut f: Frame) {
        let unique = self.bus.peers.get(&id).and_then(|p| p.unique.clone());
        let Some(unique) = unique else {
            // Before Hello only Hello is accepted.
            let is_hello = f.msg.mtype == METHOD_CALL
                && f.msg.member.as_deref() == Some("Hello")
                && f.msg.interface.as_deref() == Some(crate::bus::BUS_NAME);
            if is_hello {
                self.bus.hello(id, &f.msg);
            } else if let Some(p) = self.bus.peers.get_mut(&id) {
                p.dead = true;
            }
            return;
        };
        f.msg.sender = Some(unique);
        match f.msg.mtype {
            METHOD_CALL => self.route_call(id, &f),
            METHOD_RETURN | ERROR => {
                if let Some(Owner::Conn(t)) = f.dest.as_deref().and_then(|d| self.bus.resolve(d)) {
                    self.bus.forward(t, &f);
                }
            }
            SIGNAL => self.route_signal(id, &f),
            _ => {}
        }
    }

    fn route_call(&mut self, id: usize, f: &Frame) {
        if is_bus_call(f) {
            self.bus.bus_call(id, &f.msg);
            return;
        }
        let Some(dest) = f.dest.clone() else {
            self.bus.error(
                crate::bus::BUS_NAME,
                &f.msg,
                "org.freedesktop.DBus.Error.UnknownObject",
                f.msg.path.as_deref().unwrap_or(""),
            );
            return;
        };
        match self.bus.resolve(&dest) {
            Some(Owner::Internal) => self.ibus.handle_call(&mut self.bus, &f.msg),
            Some(Owner::Conn(t)) => self.bus.forward(t, f),
            None => self.bus.error(
                crate::bus::BUS_NAME,
                &f.msg,
                "org.freedesktop.DBus.Error.ServiceUnknown",
                &format!("The name {dest} was not provided by any .service files"),
            ),
        }
    }

    fn route_signal(&mut self, id: usize, f: &Frame) {
        // Menu clicks: PropertyActivate from whoever owns the panel name.
        let from_panel = self.bus.resolve(PANEL_NAME) == Some(Owner::Conn(id));
        if from_panel
            && f.msg.member.as_deref() == Some("PropertyActivate")
            && f.msg.path.as_deref() == Some(PANEL_PATH)
            && f.msg.interface.as_deref() == Some(PANEL_IFACE)
        {
            self.ibus.on_panel_signal(&mut self.bus, &f.msg);
        }
        match f.dest.as_deref() {
            Some(d) => {
                if let Some(Owner::Conn(t)) = self.bus.resolve(d) {
                    self.bus.forward(t, f);
                }
            }
            None => self.bus.broadcast_frame(f),
        }
    }
}

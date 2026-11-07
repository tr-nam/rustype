//! The IBus side: factory + engine objects on the IBus bus, one message at a time,
//! in arrival order. Every key is answered with one write (signals, then the reply).

use crate::config::Watcher;
use crate::dbus::{Connection, Message, Writer, METHOD_CALL};
use crate::ime::{Ime, Out};
use std::collections::HashMap;
use std::io;

pub const COMPONENT_NAME: &str = "org.freedesktop.IBus.Rustype";
pub const ENGINE_NAME: &str = "rustype";
const FACTORY_PATH: &str = "/org/freedesktop/IBus/Factory";
const ENGINE_IFACE: &str = "org.freedesktop.IBus.Engine";
const FACTORY_IFACE: &str = "org.freedesktop.IBus.Factory";
const SERVICE_IFACE: &str = "org.freedesktop.IBus.Service";
const PROPS_IFACE: &str = "org.freedesktop.DBus.Properties";
const PREEDIT_CLEAR: u32 = 0;
const PREEDIT_COMMIT: u32 = 1;
const ATTR_UNDERLINE: u32 = 1;
const ATTR_UNDERLINE_SINGLE: u32 = 1;

/// Writes an IBusText as a variant: v -> (sa{sv}sv) with an IBusAttrList (sa{sv}av) inside.
fn write_text(w: &mut Writer, text: &str, underline: bool) {
    w.signature("(sa{sv}sv)");
    w.struct_start();
    w.string("IBusText");
    let a = w.array_start(8);
    w.array_end(a);
    w.string(text);
    w.signature("(sa{sv}av)");
    w.struct_start();
    w.string("IBusAttrList");
    let a = w.array_start(8);
    w.array_end(a);
    let attrs = w.array_start(1);
    if underline && !text.is_empty() {
        w.signature("(sa{sv}uuuu)");
        w.struct_start();
        w.string("IBusAttribute");
        let a = w.array_start(8);
        w.array_end(a);
        w.u32(ATTR_UNDERLINE);
        w.u32(ATTR_UNDERLINE_SINGLE);
        w.u32(0);
        w.u32(text.chars().count() as u32);
    }
    w.array_end(attrs);
}

fn bool_variant(w: &mut Writer, v: bool) {
    w.signature("b");
    w.boolean(v);
}

pub struct Service {
    conn: Connection,
    engines: HashMap<String, Ime>,
    next_id: u32,
    config: Watcher,
    debug: bool,
}

pub fn run(address: &str) -> io::Result<()> {
    let mut conn = Connection::open(address)?;
    match conn.request_name(COMPONENT_NAME)? {
        1 | 4 => {}
        other => return Err(io::Error::other(format!("cannot own {COMPONENT_NAME} (reply {other})"))),
    }
    let mut svc = Service {
        conn,
        engines: HashMap::new(),
        next_id: 0,
        config: Watcher::new(),
        debug: std::env::var_os("RUSTYPE_DEBUG").is_some(),
    };
    let mut batch = Vec::with_capacity(512);
    loop {
        let msg = svc.conn.next()?;
        if msg.mtype != METHOD_CALL {
            continue;
        }
        batch.clear();
        let keep_running = svc.dispatch(&msg, &mut batch);
        svc.conn.flush(&batch)?;
        if !keep_running {
            return Ok(());
        }
    }
}

impl Service {
    fn emit(&mut self, batch: &mut Vec<u8>, path: &str, outs: Vec<Out>) {
        for o in outs {
            if self.debug {
                eprintln!("rustype: {path} -> {o:?}");
            }
            let mut w = Writer::new();
            let (member, sig) = match o {
                Out::Commit(t) => {
                    write_text(&mut w, &t, false);
                    ("CommitText", "v")
                }
                Out::Preedit(t) => {
                    write_text(&mut w, &t, true);
                    w.u32(t.chars().count() as u32);
                    w.boolean(true);
                    w.u32(PREEDIT_COMMIT);
                    ("UpdatePreeditText", "vubu")
                }
                Out::HidePreedit => {
                    // An empty CLEAR-mode preedit resets what IBus and the app keep, so a
                    // later reset can never commit an old word a second time.
                    write_text(&mut w, "", false);
                    w.u32(0);
                    w.boolean(false);
                    w.u32(PREEDIT_CLEAR);
                    ("UpdatePreeditText", "vubu")
                }
                Out::DeleteSurrounding { offset, nchars } => {
                    w.i32(offset);
                    w.u32(nchars);
                    ("DeleteSurroundingText", "iu")
                }
                Out::ForwardKey { keyval, keycode, state } => {
                    w.u32(keyval);
                    w.u32(keycode);
                    w.u32(state);
                    ("ForwardKeyEvent", "uuu")
                }
            };
            self.conn.signal(batch, path, ENGINE_IFACE, member, sig, &w.buf);
        }
    }

    /// Returns false when the service should exit.
    fn dispatch(&mut self, msg: &Message, batch: &mut Vec<u8>) -> bool {
        let path = msg.path.clone().unwrap_or_default();
        let iface = msg.interface.as_deref().unwrap_or("");
        let member = msg.member.as_deref().unwrap_or("");
        if iface == "org.freedesktop.DBus.Peer" && member == "Ping" {
            self.conn.reply(batch, msg, "", &[]);
            return true;
        }
        if path == FACTORY_PATH {
            return self.factory(msg, iface, member, batch);
        }
        if self.engines.contains_key(&path) {
            if let Err(e) = self.engine(msg, &path, iface, member, batch) {
                self.conn.error(batch, msg, "org.freedesktop.DBus.Error.InvalidArgs", &e.to_string());
            }
            return true;
        }
        self.conn.error(batch, msg, "org.freedesktop.DBus.Error.UnknownObject", &path);
        true
    }

    fn factory(&mut self, msg: &Message, iface: &str, member: &str, batch: &mut Vec<u8>) -> bool {
        match (iface, member) {
            (FACTORY_IFACE, "CreateEngine") => {
                let name = msg.body_reader().string().unwrap_or_default();
                if name != ENGINE_NAME {
                    self.conn.error(batch, msg, "org.freedesktop.DBus.Error.InvalidArgs", "unknown engine");
                    return true;
                }
                self.config.refresh();
                self.next_id += 1;
                let path = format!("/org/freedesktop/IBus/Engine/{}", self.next_id);
                self.engines.insert(path.clone(), Ime::new(&self.config.current));
                let mut w = Writer::new();
                w.string(&path);
                self.conn.reply(batch, msg, "o", &w.buf);
                true
            }
            (SERVICE_IFACE, "Destroy") => {
                self.conn.reply(batch, msg, "", &[]);
                false
            }
            _ => {
                self.conn.error(batch, msg, "org.freedesktop.DBus.Error.UnknownMethod", member);
                true
            }
        }
    }

    fn engine(&mut self, msg: &Message, path: &str, iface: &str, member: &str, batch: &mut Vec<u8>) -> io::Result<()> {
        let mut outs = Vec::new();
        match (iface, member) {
            (ENGINE_IFACE, "ProcessKeyEvent") => {
                let mut r = msg.body_reader();
                let (keyval, keycode, state) = (r.u32()?, r.u32()?, r.u32()?);
                let ime = self.engines.get_mut(path).unwrap();
                let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut o = Vec::new();
                    let h = ime.process_key(keyval, keycode, state, &mut o);
                    (h, o)
                }));
                let handled = match handled {
                    Ok((h, o)) => {
                        outs = o;
                        h
                    }
                    Err(_) => {
                        // Never take typing down: start the word over and let the app have the key.
                        ime.recover(&self.config.current);
                        outs = vec![Out::HidePreedit];
                        false
                    }
                };
                if self.debug {
                    eprintln!("rustype: key {keyval:#x} state {state:#x} -> {handled}");
                }
                self.emit(batch, path, outs);
                let mut w = Writer::new();
                w.boolean(handled);
                self.conn.reply(batch, msg, "b", &w.buf);
                return Ok(());
            }
            (ENGINE_IFACE, "FocusIn" | "FocusInId") => {
                self.config.refresh();
                let cfg = self.config.current.clone();
                let ime = self.engines.get_mut(path).unwrap();
                ime.reset(&mut outs);
                ime.apply_config(&cfg);
            }
            (ENGINE_IFACE, "FocusOut" | "FocusOutId" | "Reset" | "Disable") => {
                self.engines.get_mut(path).unwrap().reset(&mut outs);
            }
            (ENGINE_IFACE, "SetCapabilities") => {
                let caps = msg.body_reader().u32()?;
                self.engines.get_mut(path).unwrap().set_capabilities(caps);
            }
            (
                ENGINE_IFACE,
                "Enable" | "SetCursorLocation" | "SetSurroundingText" | "PropertyActivate" | "PropertyShow"
                | "PropertyHide" | "CandidateClicked" | "PageUp" | "PageDown" | "CursorUp" | "CursorDown"
                | "ProcessHandWritingEvent" | "CancelHandWriting" | "PanelExtensionReceived"
                | "PanelExtensionRegisterKeys",
            ) => {}
            (PROPS_IFACE, "Get") => {
                let mut r = msg.body_reader();
                let _iface = r.string()?;
                let prop = r.string()?;
                if prop == "FocusId" || prop == "ActiveSurroundingText" {
                    let mut w = Writer::new();
                    bool_variant(&mut w, false);
                    self.conn.reply(batch, msg, "v", &w.buf);
                } else {
                    self.conn.error(batch, msg, "org.freedesktop.DBus.Error.InvalidArgs", &prop);
                }
                return Ok(());
            }
            (PROPS_IFACE, "GetAll") => {
                let mut w = Writer::new();
                let arr = w.array_start(8);
                for name in ["FocusId", "ActiveSurroundingText"] {
                    w.struct_start();
                    w.string(name);
                    bool_variant(&mut w, false);
                }
                w.array_end(arr);
                self.conn.reply(batch, msg, "a{sv}", &w.buf);
                return Ok(());
            }
            (PROPS_IFACE, "Set") => {
                let mut r = msg.body_reader();
                let _iface = r.string()?;
                let prop = r.string()?;
                let sig = r.signature()?;
                if prop == "ContentType" && sig == "(uu)" {
                    r.align(8);
                    let (purpose, hints) = (r.u32()?, r.u32()?);
                    self.engines.get_mut(path).unwrap().set_content_type(purpose, hints);
                }
            }
            (SERVICE_IFACE, "Destroy") => {
                self.engines.remove(path);
            }
            _ => {
                self.conn.error(batch, msg, "org.freedesktop.DBus.Error.UnknownMethod", member);
                return Ok(());
            }
        }
        self.emit(batch, path, outs);
        self.conn.reply(batch, msg, "", &[]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus::Reader;

    #[test]
    fn ibus_text_layout() {
        let mut w = Writer::new();
        write_text(&mut w, "việt", true);
        let end = w.buf.len();
        let mut r = Reader::new(&w.buf, false);
        let mut i = 0;
        r.skip(b"v", &mut i).unwrap();
        assert_eq!(r.pos, end);
    }
}

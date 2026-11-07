//! The IBus side: factory + engine objects on the IBus bus, one message at a time,
//! in arrival order. Every key is answered with one write (signals, then the reply).

use crate::config::Watcher;
use crate::dbus::{Connection, Message, Writer, METHOD_CALL};
use crate::ime::{Ime, Out};
use std::collections::HashMap;
use std::io;

pub const COMPONENT_NAME: &str = "org.freedesktop.IBus.Rustype";
pub const ENGINE_NAME: &str = "rustype";
pub const RUSTYPE_IFACE: &str = "org.freedesktop.IBus.Rustype";
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

#[allow(dead_code)]
pub const PROP_TYPE_NORMAL: u32 = 0;
pub const PROP_TYPE_TOGGLE: u32 = 1;
pub const PROP_TYPE_RADIO: u32 = 2;
#[allow(dead_code)]
pub const PROP_TYPE_MENU: u32 = 3;
pub const PROP_TYPE_SEPARATOR: u32 = 4;

pub const PROP_STATE_UNCHECKED: u32 = 0;
pub const PROP_STATE_CHECKED: u32 = 1;

fn bool_variant(w: &mut Writer, v: bool) {
    w.signature("b");
    w.boolean(v);
}

fn write_prop(w: &mut Writer, key: &str, ptype: u32, label: &str, icon: &str, state: u32) {
    w.signature("(sa{sv}suvsvbbuvv)");
    w.struct_start();
    w.string("IBusProperty");
    let a = w.array_start(8);
    w.array_end(a);
    w.string(key);
    w.u32(ptype);
    write_text(w, label, false);
    w.string(icon);
    write_text(w, label, false);
    w.boolean(true);
    w.boolean(true);
    w.u32(state);
    // sub_props: empty IBusPropList
    w.signature("(sa{sv}av)");
    w.struct_start();
    w.string("IBusPropList");
    let a1 = w.array_start(8);
    w.array_end(a1);
    let a2 = w.array_start(1);
    w.array_end(a2);
    // symbol: empty IBusText or label for InputMode
    write_text(w, label, false);
}

fn write_prop_list(w: &mut Writer, enabled: bool, method: rustype_core::telex::InputMethod) {
    w.signature("(sa{sv}av)");
    w.struct_start();
    w.string("IBusPropList");
    let a = w.array_start(8);
    w.array_end(a);
    let props = w.array_start(1);

    // 1. Toggle switch: Bật tiếng Việt
    let toggle_label = if enabled {
        "Bật tiếng Việt"
    } else {
        "Bật tiếng Việt (Đang tắt)"
    };
    let toggle_state = if enabled {
        PROP_STATE_CHECKED
    } else {
        PROP_STATE_UNCHECKED
    };
    write_prop(
        w,
        "prop_toggle",
        PROP_TYPE_TOGGLE,
        toggle_label,
        "",
        toggle_state,
    );

    // 2. Separator
    write_prop(w, "prop_sep1", PROP_TYPE_SEPARATOR, "", "", 0);

    // 3. Radio Telex
    let telex_state = if method == rustype_core::telex::InputMethod::Telex {
        PROP_STATE_CHECKED
    } else {
        PROP_STATE_UNCHECKED
    };
    write_prop(
        w,
        "prop_telex",
        PROP_TYPE_RADIO,
        "Kiểu gõ Telex",
        "",
        telex_state,
    );

    // 4. Radio VNI
    let vni_state = if method == rustype_core::telex::InputMethod::Vni {
        PROP_STATE_CHECKED
    } else {
        PROP_STATE_UNCHECKED
    };
    write_prop(w, "prop_vni", PROP_TYPE_RADIO, "Kiểu gõ VNI", "", vni_state);

    // 5. Separator
    write_prop(w, "prop_sep2", PROP_TYPE_SEPARATOR, "", "", 0);

    // 6. Settings
    write_prop(
        w,
        "prop_settings",
        PROP_TYPE_NORMAL,
        "Cài đặt Rustype...",
        "preferences-system",
        0,
    );

    // 7. InputMode (Dynamically sets panel text to "VN" or "EN" in GNOME Shell)
    let mode_str = if enabled { "VN" } else { "EN" };
    write_prop(w, "InputMode", PROP_TYPE_NORMAL, mode_str, "", 0);

    w.array_end(props);
}

#[allow(clippy::too_many_arguments)]
fn emit_update_prop(
    conn: &mut Connection,
    batch: &mut Vec<u8>,
    path: &str,
    key: &str,
    ptype: u32,
    label: &str,
    icon: &str,
    state: u32,
) {
    let mut pw = Writer::new();
    write_prop(&mut pw, key, ptype, label, icon, state);
    conn.signal(batch, path, ENGINE_IFACE, "UpdateProperty", "v", &pw.buf);
}

pub struct Service {
    conn: Connection,
    engines: HashMap<String, Ime>,
    next_id: u32,
    config: Watcher,
    debug: bool,
    current_app_id: Option<String>,
    app_states: HashMap<String, bool>,
}

pub fn run(address: &str) -> io::Result<()> {
    let mut conn = Connection::open(address)?;
    match conn.request_name(COMPONENT_NAME)? {
        1 | 4 => {}
        other => {
            return Err(io::Error::other(format!(
                "cannot own {COMPONENT_NAME} (reply {other})"
            )))
        }
    }
    let mut svc = Service {
        conn,
        engines: HashMap::new(),
        next_id: 0,
        config: Watcher::new(),
        debug: std::env::var_os("RUSTYPE_DEBUG").is_some(),
        current_app_id: None,
        app_states: HashMap::new(),
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
                let label = match &o {
                    Out::Commit(t) => format!("Commit({} chars)", t.chars().count()),
                    Out::Preedit(t) => format!("Preedit({} chars)", t.chars().count()),
                    Out::HidePreedit => "HidePreedit".into(),
                    Out::DeleteSurrounding { offset, nchars } => {
                        format!("DeleteSurrounding({offset}, {nchars})")
                    }
                    Out::ForwardKey {
                        keyval,
                        keycode,
                        state,
                    } => format!("ForwardKey({keyval:#x}, {keycode}, {state:#x})"),
                };
                eprintln!("rustype: {path} -> {label}");
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
                Out::ForwardKey {
                    keyval,
                    keycode,
                    state,
                } => {
                    w.u32(keyval);
                    w.u32(keycode);
                    w.u32(state);
                    ("ForwardKeyEvent", "uuu")
                }
            };
            self.conn
                .signal(batch, path, ENGINE_IFACE, member, sig, &w.buf);
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
        if iface == RUSTYPE_IFACE && member == "SetFocusApp" {
            let app_id = msg.body_reader().string().ok();
            self.current_app_id = app_id.clone();
            let enabled = if self.config.current.remember_state_per_app {
                if let Some(id) = &app_id {
                    *self.app_states.entry(id.clone()).or_insert(true)
                } else {
                    true
                }
            } else {
                true
            };
            for ime in self.engines.values_mut() {
                let mut outs = Vec::new();
                ime.set_app_id(app_id.clone(), &self.config.current);
                ime.set_enabled(enabled, &mut outs);
            }
            self.conn.reply(batch, msg, "", &[]);
            return true;
        }
        if iface == RUSTYPE_IFACE && member == "ToggleState" {
            let mut new_state = true;
            if let Some(app_id) = &self.current_app_id {
                let cur = self.app_states.entry(app_id.clone()).or_insert(true);
                *cur = !*cur;
                new_state = *cur;
            }
            let mut all_outs = Vec::new();
            for (path, ime) in self.engines.iter_mut() {
                let mut outs = Vec::new();
                ime.set_enabled(new_state, &mut outs);
                if !outs.is_empty() {
                    all_outs.push((path.clone(), outs));
                }
            }
            for (path, outs) in all_outs {
                self.emit(batch, &path, outs);
            }
            let mut w = Writer::new();
            w.boolean(new_state);
            self.conn.reply(batch, msg, "b", &w.buf);
            return true;
        }
        if iface == RUSTYPE_IFACE && member == "GetState" {
            let state = if let Some(app_id) = &self.current_app_id {
                *self.app_states.get(app_id).unwrap_or(&true)
            } else {
                true
            };
            let mut w = Writer::new();
            w.boolean(state);
            self.conn.reply(batch, msg, "b", &w.buf);
            return true;
        }
        if path == FACTORY_PATH {
            return self.factory(msg, iface, member, batch);
        }
        if self.engines.contains_key(&path) {
            if let Err(e) = self.engine(msg, &path, iface, member, batch) {
                self.conn.error(
                    batch,
                    msg,
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    &e.to_string(),
                );
            }
            return true;
        }
        self.conn.error(
            batch,
            msg,
            "org.freedesktop.DBus.Error.UnknownObject",
            &path,
        );
        true
    }

    fn factory(&mut self, msg: &Message, iface: &str, member: &str, batch: &mut Vec<u8>) -> bool {
        match (iface, member) {
            (FACTORY_IFACE, "CreateEngine") => {
                let name = msg.body_reader().string().unwrap_or_default();
                if name != ENGINE_NAME {
                    self.conn.error(
                        batch,
                        msg,
                        "org.freedesktop.DBus.Error.InvalidArgs",
                        "unknown engine",
                    );
                    return true;
                }
                self.config.refresh();
                self.next_id += 1;
                let path = format!("/org/freedesktop/IBus/Engine/{}", self.next_id);
                let mut ime = Ime::new(&self.config.current);
                ime.set_app_id(self.current_app_id.clone(), &self.config.current);
                self.engines.insert(path.clone(), ime);
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
                self.conn.error(
                    batch,
                    msg,
                    "org.freedesktop.DBus.Error.UnknownMethod",
                    member,
                );
                true
            }
        }
    }

    fn engine(
        &mut self,
        msg: &Message,
        path: &str,
        iface: &str,
        member: &str,
        batch: &mut Vec<u8>,
    ) -> io::Result<()> {
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
            (ENGINE_IFACE, "Enable") => {
                self.config.refresh();
                let cfg = self.config.current.clone();
                let app_id = self.current_app_id.clone();
                let enabled = if cfg.remember_state_per_app {
                    if let Some(id) = &app_id {
                        *self.app_states.entry(id.clone()).or_insert(true)
                    } else {
                        true
                    }
                } else {
                    true
                };
                if let Some(ime) = self.engines.get_mut(path) {
                    ime.reset(&mut outs);
                    ime.set_app_id(app_id, &cfg);
                    ime.set_enabled(enabled, &mut outs);
                }
                let mut pw = Writer::new();
                write_prop_list(&mut pw, enabled, cfg.telex.input_method);
                self.conn.signal(
                    batch,
                    path,
                    ENGINE_IFACE,
                    "RegisterProperties",
                    "v",
                    &pw.buf,
                );
            }
            (ENGINE_IFACE, "FocusIn" | "FocusInId") => {
                self.config.refresh();
                let cfg = self.config.current.clone();
                let app_id = self.current_app_id.clone();
                let enabled = if cfg.remember_state_per_app {
                    if let Some(id) = &app_id {
                        *self.app_states.entry(id.clone()).or_insert(true)
                    } else {
                        true
                    }
                } else {
                    true
                };
                let ime = self.engines.get_mut(path).unwrap();
                ime.reset(&mut outs);
                ime.set_app_id(app_id, &cfg);
                ime.set_enabled(enabled, &mut outs);

                // Register property list on focus
                let mut pw = Writer::new();
                write_prop_list(&mut pw, enabled, cfg.telex.input_method);
                self.conn.signal(
                    batch,
                    path,
                    ENGINE_IFACE,
                    "RegisterProperties",
                    "v",
                    &pw.buf,
                );

                // Update switch & panel indicator for current app state
                let toggle_label = if enabled {
                    "Bật tiếng Việt"
                } else {
                    "Bật tiếng Việt (Đang tắt)"
                };
                let toggle_state = if enabled {
                    PROP_STATE_CHECKED
                } else {
                    PROP_STATE_UNCHECKED
                };
                emit_update_prop(
                    &mut self.conn,
                    batch,
                    path,
                    "prop_toggle",
                    PROP_TYPE_TOGGLE,
                    toggle_label,
                    "",
                    toggle_state,
                );
                let mode_str = if enabled { "VN" } else { "EN" };
                emit_update_prop(
                    &mut self.conn,
                    batch,
                    path,
                    "InputMode",
                    PROP_TYPE_NORMAL,
                    mode_str,
                    "",
                    0,
                );
            }
            (ENGINE_IFACE, "FocusOut" | "FocusOutId" | "Reset" | "Disable") => {
                self.engines.get_mut(path).unwrap().reset(&mut outs);
            }
            (ENGINE_IFACE, "SetCapabilities") => {
                let caps = msg.body_reader().u32()?;
                self.engines.get_mut(path).unwrap().set_capabilities(caps);
            }
            (ENGINE_IFACE, "SetSurroundingText") => {
                let mut r = msg.body_reader();
                if let Ok(text) = r.read_ibus_text() {
                    let cursor = r.u32().unwrap_or(0);
                    let anchor = r.u32().unwrap_or(0);
                    let ime = self.engines.get_mut(path).unwrap();
                    ime.set_surrounding_text(&text, cursor, anchor, &mut outs);
                }
            }
            (ENGINE_IFACE, "PropertyActivate") => {
                let mut r = msg.body_reader();
                let name = r.string().unwrap_or_default();
                let state = r.u32().unwrap_or(0);
                match name.as_str() {
                    "prop_toggle" => {
                        let ime = self.engines.get_mut(path).unwrap();
                        let cur_enabled = ime.is_enabled();
                        let new_state = if state == PROP_STATE_CHECKED {
                            true
                        } else if state == PROP_STATE_UNCHECKED {
                            false
                        } else {
                            !cur_enabled
                        };
                        if let Some(app_id) = &self.current_app_id {
                            self.app_states.insert(app_id.clone(), new_state);
                        }
                        ime.set_enabled(new_state, &mut outs);

                        let toggle_label = if new_state {
                            "Bật tiếng Việt"
                        } else {
                            "Bật tiếng Việt (Đang tắt)"
                        };
                        let toggle_state = if new_state {
                            PROP_STATE_CHECKED
                        } else {
                            PROP_STATE_UNCHECKED
                        };
                        emit_update_prop(
                            &mut self.conn,
                            batch,
                            path,
                            "prop_toggle",
                            PROP_TYPE_TOGGLE,
                            toggle_label,
                            "",
                            toggle_state,
                        );
                        let mode_str = if new_state { "VN" } else { "EN" };
                        emit_update_prop(
                            &mut self.conn,
                            batch,
                            path,
                            "InputMode",
                            PROP_TYPE_NORMAL,
                            mode_str,
                            "",
                            0,
                        );
                    }
                    "prop_telex" => {
                        self.config.current.telex.input_method =
                            rustype_core::telex::InputMethod::Telex;
                        crate::config::update_input_method(rustype_core::telex::InputMethod::Telex);
                        for engine in self.engines.values_mut() {
                            engine.apply_config(&self.config.current);
                        }
                        emit_update_prop(
                            &mut self.conn,
                            batch,
                            path,
                            "prop_telex",
                            PROP_TYPE_RADIO,
                            "Kiểu gõ Telex",
                            "",
                            PROP_STATE_CHECKED,
                        );
                        emit_update_prop(
                            &mut self.conn,
                            batch,
                            path,
                            "prop_vni",
                            PROP_TYPE_RADIO,
                            "Kiểu gõ VNI",
                            "",
                            PROP_STATE_UNCHECKED,
                        );
                    }
                    "prop_vni" => {
                        self.config.current.telex.input_method =
                            rustype_core::telex::InputMethod::Vni;
                        crate::config::update_input_method(rustype_core::telex::InputMethod::Vni);
                        for engine in self.engines.values_mut() {
                            engine.apply_config(&self.config.current);
                        }
                        emit_update_prop(
                            &mut self.conn,
                            batch,
                            path,
                            "prop_telex",
                            PROP_TYPE_RADIO,
                            "Kiểu gõ Telex",
                            "",
                            PROP_STATE_UNCHECKED,
                        );
                        emit_update_prop(
                            &mut self.conn,
                            batch,
                            path,
                            "prop_vni",
                            PROP_TYPE_RADIO,
                            "Kiểu gõ VNI",
                            "",
                            PROP_STATE_CHECKED,
                        );
                    }
                    "prop_method" => {
                        let next_method = match self.config.current.telex.input_method {
                            rustype_core::telex::InputMethod::Telex => {
                                rustype_core::telex::InputMethod::Vni
                            }
                            rustype_core::telex::InputMethod::Vni => {
                                rustype_core::telex::InputMethod::Telex
                            }
                        };
                        self.config.current.telex.input_method = next_method;
                        crate::config::update_input_method(next_method);
                        for engine in self.engines.values_mut() {
                            engine.apply_config(&self.config.current);
                        }
                        let telex_state = if next_method == rustype_core::telex::InputMethod::Telex
                        {
                            PROP_STATE_CHECKED
                        } else {
                            PROP_STATE_UNCHECKED
                        };
                        let vni_state = if next_method == rustype_core::telex::InputMethod::Vni {
                            PROP_STATE_CHECKED
                        } else {
                            PROP_STATE_UNCHECKED
                        };
                        emit_update_prop(
                            &mut self.conn,
                            batch,
                            path,
                            "prop_telex",
                            PROP_TYPE_RADIO,
                            "Kiểu gõ Telex",
                            "",
                            telex_state,
                        );
                        emit_update_prop(
                            &mut self.conn,
                            batch,
                            path,
                            "prop_vni",
                            PROP_TYPE_RADIO,
                            "Kiểu gõ VNI",
                            "",
                            vni_state,
                        );
                    }
                    "prop_settings" => {
                        let candidates = [
                            "/usr/bin/rustype-settings".to_string(),
                            format!(
                                "{}/.local/bin/rustype-settings",
                                std::env::var("HOME").unwrap_or_default()
                            ),
                        ];
                        let mut spawned = false;
                        for exe in &candidates {
                            if std::path::Path::new(exe).exists()
                                && std::process::Command::new(exe).spawn().is_ok()
                            {
                                spawned = true;
                                break;
                            }
                        }
                        if !spawned {
                            let _ = std::process::Command::new("rustype-settings").spawn();
                        }
                    }
                    _ => {}
                }
            }
            (
                ENGINE_IFACE,
                "SetCursorLocation"
                | "PropertyShow"
                | "PropertyHide"
                | "CandidateClicked"
                | "PageUp"
                | "PageDown"
                | "CursorUp"
                | "CursorDown"
                | "ProcessHandWritingEvent"
                | "CancelHandWriting"
                | "PanelExtensionReceived"
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
                    self.conn
                        .error(batch, msg, "org.freedesktop.DBus.Error.InvalidArgs", &prop);
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
                    self.engines
                        .get_mut(path)
                        .unwrap()
                        .set_content_type(purpose, hints);
                }
            }
            (SERVICE_IFACE, "Destroy") => {
                self.engines.remove(path);
            }
            _ => {
                self.conn.error(
                    batch,
                    msg,
                    "org.freedesktop.DBus.Error.UnknownMethod",
                    member,
                );
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

    #[test]
    fn ibus_prop_list_layout() {
        let mut w = Writer::new();
        write_prop_list(&mut w, true, rustype_core::telex::InputMethod::Telex);
        let end = w.buf.len();
        let mut r = Reader::new(&w.buf, false);
        let mut i = 0;
        r.skip(b"v", &mut i).unwrap();
        assert_eq!(r.pos, end);

        let fields = vec![
            crate::dbus::Field::Path("/org/freedesktop/IBus/Engine/1"),
            crate::dbus::Field::Interface(ENGINE_IFACE),
            crate::dbus::Field::Member("RegisterProperties"),
        ];
        let blob = crate::dbus::encode(crate::dbus::SIGNAL, 0, 1, &fields, "v", &w.buf);
        let msg = crate::dbus::read_message(&mut &blob[..]).unwrap();
        assert_eq!(msg.member.as_deref(), Some("RegisterProperties"));
        assert_eq!(msg.signature, "v");
        let mut mr = msg.body_reader();
        let mut mi = 0;
        mr.skip(b"v", &mut mi).unwrap();
        assert_eq!(mr.pos, msg.body.len());
    }

    #[test]
    fn ibus_update_property_layout() {
        let mut w = Writer::new();
        write_prop(
            &mut w,
            "prop_toggle",
            PROP_TYPE_TOGGLE,
            "Bật tiếng Việt",
            "",
            PROP_STATE_CHECKED,
        );
        let end = w.buf.len();
        let mut r = Reader::new(&w.buf, false);
        let mut i = 0;
        r.skip(b"v", &mut i).unwrap();
        assert_eq!(r.pos, end);

        let fields = vec![
            crate::dbus::Field::Path("/org/freedesktop/IBus/Engine/1"),
            crate::dbus::Field::Interface(ENGINE_IFACE),
            crate::dbus::Field::Member("UpdateProperty"),
        ];
        let blob = crate::dbus::encode(crate::dbus::SIGNAL, 0, 1, &fields, "v", &w.buf);
        let msg = crate::dbus::read_message(&mut &blob[..]).unwrap();
        assert_eq!(msg.member.as_deref(), Some("UpdateProperty"));
        assert_eq!(msg.signature, "v");
    }
}

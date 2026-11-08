//! The org.freedesktop.IBus service, in-process: the IBus object, input contexts, the panel
//! client and rustype's own interface. Key handling is rustype_ibus::ime, unchanged.

use crate::bus::{Bus, IBUS_NAME, PANEL_IFACE, PANEL_NAME, PANEL_PATH};
use crate::wire::{body_bool, body_str};
use rustype_core::telex::InputMethod;
use rustype_ibus::config::{update_input_method, Watcher};
use rustype_ibus::dbus::{Message, Reader, Writer};
use rustype_ibus::ibus::{
    app_state_mut, spawn_and_reap, write_prop, write_prop_list, write_text, PROP_STATE_CHECKED,
    PROP_STATE_UNCHECKED, PROP_TYPE_RADIO, PROP_TYPE_TOGGLE, RUSTYPE_IFACE,
};
use rustype_ibus::ime::{Ime, Out};
use std::collections::{BTreeMap, HashMap};
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const IBUS_PATH: &str = "/org/freedesktop/IBus";
const IBUS_IFACE: &str = "org.freedesktop.IBus";
const FACTORY_PATH: &str = "/org/freedesktop/IBus/Factory";
const IC_PREFIX: &str = "/org/freedesktop/IBus/InputContext_";
const IC_IFACE: &str = "org.freedesktop.IBus.InputContext";
const SERVICE_IFACE: &str = "org.freedesktop.IBus.Service";
const PROPS_IFACE: &str = "org.freedesktop.DBus.Properties";
const ENGINE_RUSTYPE: &str = "rustype";
const ENGINE_PASSTHROUGH: &str = "xkb:us::eng";
/// IBusEngineDesc as libibus deserializes it (FACTS B-section check with IBus.EngineDesc).
const DESC_SIG: &str = "(sa{sv}ssssssssussssssss)";
const MODE_COMMIT: u32 = 1;

const INVALID_ARGS: &str = "org.freedesktop.DBus.Error.InvalidArgs";
const UNKNOWN_METHOD: &str = "org.freedesktop.DBus.Error.UnknownMethod";
const UNKNOWN_OBJECT: &str = "org.freedesktop.DBus.Error.UnknownObject";
const UNKNOWN_PROPERTY: &str = "org.freedesktop.DBus.Error.UnknownProperty";

/// A failed call: the D-Bus error name and its text.
#[derive(Debug)]
pub struct Fail(pub &'static str, pub String);

impl From<io::Error> for Fail {
    fn from(e: io::Error) -> Fail {
        Fail(INVALID_ARGS, e.to_string())
    }
}

type R = Result<(), Fail>;

struct EngineInfo {
    name: &'static str,
    longname: &'static str,
    description: &'static str,
    language: &'static str,
    license: &'static str,
    author: &'static str,
    icon: &'static str,
    layout: &'static str,
}

const ENGINES: [EngineInfo; 2] = [
    EngineInfo {
        name: ENGINE_RUSTYPE,
        longname: "Rustype",
        description: "Bộ gõ siêu nhẹ chạy bằng Rust",
        language: "vi",
        license: "MIT",
        author: "NamTr_n",
        icon: "rustype",
        layout: "us",
    },
    EngineInfo {
        name: ENGINE_PASSTHROUGH,
        longname: "English (US)",
        description: "English (US)",
        language: "en",
        license: "GPL",
        author: "Ubuntu",
        icon: "",
        layout: "us",
    },
];

fn engine_info(name: &str) -> &'static EngineInfo {
    ENGINES
        .iter()
        .find(|e| e.name == name)
        .unwrap_or(&ENGINES[1])
}

fn engine_known(name: &str) -> bool {
    ENGINES.iter().any(|e| e.name == name)
}

/// One IBusEngineDesc, written as the body of an `(sa{sv}...)` variant value.
fn write_desc(w: &mut Writer, e: &EngineInfo) {
    w.struct_start();
    w.string("IBusEngineDesc");
    let attachments = w.array_start(8);
    w.array_end(attachments);
    for s in [
        e.name,
        e.longname,
        e.description,
        e.language,
        e.license,
        e.author,
        e.icon,
        e.layout,
    ] {
        w.string(s);
    }
    w.u32(0); // rank
    for _ in 0..8 {
        w.string("");
    }
}

fn write_engine_array(w: &mut Writer) {
    let arr = w.array_start(1);
    for e in &ENGINES {
        w.signature(DESC_SIG);
        write_desc(w, e);
    }
    w.array_end(arr);
}

fn input_mode_body(enabled: bool) -> Vec<u8> {
    let (state, sym) = if enabled {
        (PROP_STATE_CHECKED, "VN")
    } else {
        (PROP_STATE_UNCHECKED, "EN")
    };
    let mut w = Writer::new();
    write_prop(
        &mut w,
        "InputMode",
        PROP_TYPE_TOGGLE,
        "Bật tiếng Việt",
        "",
        state,
        sym,
    );
    w.buf
}

fn radio_body(key: &str, label: &str, checked: bool) -> Vec<u8> {
    let state = if checked {
        PROP_STATE_CHECKED
    } else {
        PROP_STATE_UNCHECKED
    };
    let mut w = Writer::new();
    write_prop(&mut w, key, PROP_TYPE_RADIO, label, "", state, "");
    w.buf
}

/// One input context: a client's text field, with its engine (rustype's Ime, or passthrough).
struct Ctx {
    owner_unique: String,
    engine: String,
    ime: Option<Ime>,
    /// Client state that outlives the engine: capabilities and (purpose, hints).
    caps: u32,
    content: (u32, u32),
    client_commit: bool,
    /// The COMMIT-mode preedit currently shown, which the daemon commits on reset/focus-out
    /// when the client does not do that itself (FACTS B10).
    preedit: Option<String>,
}

pub struct IBusService {
    ctxs: BTreeMap<String, Ctx>,
    focused: Option<String>,
    next_ic: u64,
    global: String,
    config: Watcher,
    current_app: Option<String>,
    app_states: HashMap<String, bool>,
    address: String,
    debug: bool,
    pub exit_requested: bool,
}

impl IBusService {
    pub fn new(address: String) -> IBusService {
        IBusService {
            ctxs: BTreeMap::new(),
            focused: None,
            next_ic: 0,
            global: ENGINE_PASSTHROUGH.to_string(),
            config: Watcher::new(),
            current_app: None,
            app_states: HashMap::new(),
            address,
            debug: std::env::var_os("RUSTYPE_DEBUG").is_some(),
            exit_requested: false,
        }
    }

    // ------------------------------------------------------------ dispatch

    /// Handles a method call addressed to org.freedesktop.IBus (or its Rustype/Factory object).
    pub fn handle_call(&mut self, bus: &mut Bus, m: &Message) {
        let path = m.path.clone().unwrap_or_default();
        let iface = m.interface.clone().unwrap_or_default();
        let member = m.member.clone().unwrap_or_default();
        let res = if iface == "org.freedesktop.DBus.Peer" && member == "Ping" {
            bus.reply(IBUS_NAME, m, "", &[]);
            Ok(())
        } else if iface == "org.freedesktop.DBus.Peer" && member == "GetMachineId" {
            bus.reply(IBUS_NAME, m, "s", &body_str(&bus.uuid.clone()));
            Ok(())
        } else if path == IBUS_PATH && iface == IBUS_IFACE {
            self.ibus_method(bus, m, &member)
        } else if path == IBUS_PATH && iface == PROPS_IFACE {
            self.ibus_props(bus, m, &member)
        } else if path == FACTORY_PATH && iface == RUSTYPE_IFACE {
            self.rustype_method(bus, m, &member)
        } else if path.starts_with(IC_PREFIX) && self.ctxs.contains_key(&path) {
            self.ic_call(bus, m, &path, &iface, &member)
        } else {
            Err(Fail(UNKNOWN_OBJECT, path))
        };
        if let Err(Fail(name, text)) = res {
            bus.error(IBUS_NAME, m, name, &text);
        }
    }

    /// A signal from the panel owner: PropertyActivate is a menu click for the focused context.
    pub fn on_panel_signal(&mut self, bus: &mut Bus, m: &Message) {
        let mut r = m.body_reader();
        let (Ok(name), Ok(state)) = (r.string(), r.u32()) else {
            return;
        };
        if let Some(path) = self.focused.clone() {
            self.activate(bus, &path, &name, state);
        }
    }

    /// A connection went away: its input contexts go with it.
    pub fn on_peer_gone(&mut self, bus: &mut Bus, unique: &str) {
        let gone: Vec<String> = self
            .ctxs
            .iter()
            .filter(|(_, c)| c.owner_unique == unique)
            .map(|(p, _)| p.clone())
            .collect();
        for path in gone {
            self.destroy_ic(bus, &path);
        }
    }

    fn ibus_method(&mut self, bus: &mut Bus, m: &Message, member: &str) -> R {
        let mut r = m.body_reader();
        match member {
            "CreateInputContext" => {
                let _client_name = r.string()?;
                let caller = m.sender.clone().unwrap_or_default();
                let path = self.create_ic(&caller);
                bus.reply(IBUS_NAME, m, "o", &body_str(&path));
            }
            "CurrentInputContext" => {
                let path = self.focused.clone().unwrap_or_else(|| "/".into());
                bus.reply(IBUS_NAME, m, "o", &body_str(&path));
            }
            "Ping" => bus.reply(IBUS_NAME, m, "v", &m.body),
            "GetAddress" => bus.reply(IBUS_NAME, m, "s", &body_str(&self.address)),
            "ListEngines" | "ListActiveEngines" => {
                let mut w = Writer::new();
                write_engine_array(&mut w);
                bus.reply(IBUS_NAME, m, "av", &w.buf);
            }
            "GetEnginesByNames" => {
                let names = read_string_array(&mut r)?;
                let mut w = Writer::new();
                let arr = w.array_start(1);
                for n in names.iter().filter(|n| engine_known(n)) {
                    w.signature(DESC_SIG);
                    write_desc(&mut w, engine_info(n));
                }
                w.array_end(arr);
                bus.reply(IBUS_NAME, m, "av", &w.buf);
            }
            "SetGlobalEngine" => {
                let name = r.string()?;
                if !engine_known(&name) {
                    return Err(Fail(INVALID_ARGS, format!("no such engine: {name}")));
                }
                self.set_global(bus, &name);
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "GetGlobalEngine" => {
                let mut w = Writer::new();
                w.signature(DESC_SIG);
                write_desc(&mut w, engine_info(&self.global));
                bus.reply(IBUS_NAME, m, "v", &w.buf);
            }
            "GetUseSysLayout" => bus.reply(IBUS_NAME, m, "b", &body_bool(false)),
            "GetUseGlobalEngine" | "IsGlobalEngineEnabled" => {
                bus.reply(IBUS_NAME, m, "b", &body_bool(true))
            }
            "RegisterComponent" => bus.reply(IBUS_NAME, m, "", &[]),
            "Exit" => {
                self.exit_requested = true;
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            _ => return Err(Fail(UNKNOWN_METHOD, member.to_string())),
        }
        Ok(())
    }

    /// Writes one property value as a variant body: the signature of the value, then the value.
    fn prop_value(&self, w: &mut Writer, prop: &str) -> bool {
        match prop {
            "Address" => {
                w.signature("s");
                w.string(&self.address);
            }
            "CurrentInputContext" => {
                w.signature("o");
                w.string(self.focused.as_deref().unwrap_or("/"));
            }
            "Engines" | "ActiveEngines" => {
                w.signature("av");
                write_engine_array(w);
            }
            "GlobalEngine" => {
                w.signature("v");
                w.signature(DESC_SIG);
                write_desc(w, engine_info(&self.global));
            }
            "EmbedPreeditText" => {
                w.signature("b");
                w.boolean(false);
            }
            _ => return false,
        }
        true
    }

    fn ibus_props(&mut self, bus: &mut Bus, m: &Message, member: &str) -> R {
        let mut r = m.body_reader();
        match member {
            "Get" => {
                let _iface = r.string()?;
                let prop = r.string()?;
                let mut w = Writer::new();
                if !self.prop_value(&mut w, &prop) {
                    return Err(Fail(UNKNOWN_PROPERTY, prop));
                }
                bus.reply(IBUS_NAME, m, "v", &w.buf);
            }
            "GetAll" => {
                let mut w = Writer::new();
                let arr = w.array_start(8);
                for p in [
                    "Address",
                    "CurrentInputContext",
                    "Engines",
                    "ActiveEngines",
                    "GlobalEngine",
                    "EmbedPreeditText",
                ] {
                    w.struct_start();
                    w.string(p);
                    self.prop_value(&mut w, p);
                }
                w.array_end(arr);
                bus.reply(IBUS_NAME, m, "a{sv}", &w.buf);
            }
            // PreloadEngines, GlobalShortcutKeys, EmbedPreeditText: accepted, no effect here.
            "Set" => bus.reply(IBUS_NAME, m, "", &[]),
            _ => return Err(Fail(UNKNOWN_METHOD, member.to_string())),
        }
        Ok(())
    }

    fn rustype_method(&mut self, bus: &mut Bus, m: &Message, member: &str) -> R {
        match member {
            "SetFocusApp" => {
                let app = m.body_reader().string().ok();
                self.current_app = app.clone();
                let enabled = self.app_enabled();
                let cfg = self.config.current.clone();
                let paths: Vec<String> = self.ctxs.keys().cloned().collect();
                for p in paths {
                    let mut outs = Vec::new();
                    if let Some(ime) = self.ctxs.get_mut(&p).and_then(|c| c.ime.as_mut()) {
                        ime.set_app_id(app.clone(), &cfg);
                        ime.set_enabled(enabled, &mut outs);
                    }
                    self.emit(bus, &p, outs);
                }
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "ToggleState" => {
                let new_state = match self.current_app.clone() {
                    Some(app) => {
                        let cur = app_state_mut(&mut self.app_states, &app);
                        *cur = !*cur;
                        *cur
                    }
                    None => true,
                };
                let paths: Vec<String> = self.ctxs.keys().cloned().collect();
                for p in paths {
                    let mut outs = Vec::new();
                    if let Some(ime) = self.ctxs.get_mut(&p).and_then(|c| c.ime.as_mut()) {
                        ime.set_enabled(new_state, &mut outs);
                    }
                    self.emit(bus, &p, outs);
                }
                bus.reply(IBUS_NAME, m, "b", &body_bool(new_state));
            }
            "GetState" => {
                // Read only: an app we have not seen is reported as enabled and not stored.
                let state = match &self.current_app {
                    Some(app) => *self.app_states.get(app).unwrap_or(&true),
                    None => true,
                };
                bus.reply(IBUS_NAME, m, "b", &body_bool(state));
            }
            _ => return Err(Fail(UNKNOWN_METHOD, member.to_string())),
        }
        Ok(())
    }

    fn ic_call(&mut self, bus: &mut Bus, m: &Message, path: &str, iface: &str, member: &str) -> R {
        match (iface, member) {
            (PROPS_IFACE, _) => self.ic_props(bus, m, path, member),
            (SERVICE_IFACE, "Destroy") => {
                self.destroy_ic(bus, path);
                bus.reply(IBUS_NAME, m, "", &[]);
                Ok(())
            }
            (IC_IFACE, _) => self.ic_method(bus, m, path, member),
            _ => Err(Fail(UNKNOWN_METHOD, member.to_string())),
        }
    }

    fn ic_props(&mut self, bus: &mut Bus, m: &Message, path: &str, member: &str) -> R {
        let mut r = m.body_reader();
        match member {
            "Get" => {
                let _iface = r.string()?;
                let prop = r.string()?;
                if prop != "PostProcessKeyEvent" {
                    return Err(Fail(UNKNOWN_PROPERTY, prop));
                }
                let mut w = Writer::new();
                w.signature("a(yv)");
                let arr = w.array_start(8);
                w.array_end(arr);
                bus.reply(IBUS_NAME, m, "v", &w.buf);
            }
            "GetAll" => {
                let mut w = Writer::new();
                let arr = w.array_start(8);
                w.struct_start();
                w.string("PostProcessKeyEvent");
                w.signature("a(yv)");
                let inner = w.array_start(8);
                w.array_end(inner);
                w.array_end(arr);
                bus.reply(IBUS_NAME, m, "a{sv}", &w.buf);
            }
            "Set" => {
                let _iface = r.string()?;
                let prop = r.string()?;
                let sig = r.signature()?;
                match (prop.as_str(), sig.as_str()) {
                    ("ContentType", "(uu)") => {
                        r.align(8);
                        let purpose = r.u32()?;
                        let hints = r.u32()?;
                        if let Some(c) = self.ctxs.get_mut(path) {
                            c.content = (purpose, hints);
                            if let Some(ime) = c.ime.as_mut() {
                                ime.set_content_type(purpose, hints);
                            }
                        }
                        let mut w = Writer::new();
                        w.u32(purpose);
                        w.u32(hints);
                        bus.call_name(
                            PANEL_NAME,
                            PANEL_PATH,
                            PANEL_IFACE,
                            "ContentType",
                            "uu",
                            &w.buf,
                        );
                    }
                    // libibus writes these as one-field structs, "(b)"; a bare "b" is accepted too.
                    ("ClientCommitPreedit", "(b)") | ("ClientCommitPreedit", "b") => {
                        if sig == "(b)" {
                            r.align(8);
                        }
                        let v = r.u32()? != 0;
                        if let Some(c) = self.ctxs.get_mut(path) {
                            c.client_commit = v;
                        }
                    }
                    // EffectivePostProcessKeyEvent: queueing is not needed (ProcessKeyEvent
                    // answers directly), so the flag is accepted and ignored.
                    _ => {}
                }
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            _ => return Err(Fail(UNKNOWN_METHOD, member.to_string())),
        }
        Ok(())
    }

    fn ic_method(&mut self, bus: &mut Bus, m: &Message, path: &str, member: &str) -> R {
        let mut r = m.body_reader();
        match member {
            "ProcessKeyEvent" => {
                let (kv, kc, st) = (r.u32()?, r.u32()?, r.u32()?);
                let handled = self.process_key(bus, path, kv, kc, st);
                bus.reply(IBUS_NAME, m, "b", &body_bool(handled));
            }
            "SetCursorLocation" => {
                bus.call_name(
                    PANEL_NAME,
                    PANEL_PATH,
                    PANEL_IFACE,
                    "SetCursorLocation",
                    "iiii",
                    &m.body,
                );
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "SetCursorLocationRelative" => {
                bus.call_name(
                    PANEL_NAME,
                    PANEL_PATH,
                    PANEL_IFACE,
                    "SetCursorLocationRelative",
                    "iiii",
                    &m.body,
                );
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "ProcessHandWritingEvent" | "CancelHandWriting" => bus.reply(IBUS_NAME, m, "", &[]),
            "FocusIn" => {
                self.focus_in(bus, path);
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "FocusOut" => {
                self.focus_out(bus, path);
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "Reset" => {
                self.reset_ctx(bus, path);
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "SetCapabilities" => {
                let caps = r.u32()?;
                if let Some(c) = self.ctxs.get_mut(path) {
                    c.caps = caps;
                    if let Some(ime) = c.ime.as_mut() {
                        ime.set_capabilities(caps);
                    }
                }
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "PropertyActivate" => {
                let name = r.string()?;
                let state = r.u32()?;
                self.activate(bus, path, &name, state);
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "SetEngine" => {
                let name = r.string()?;
                if !engine_known(&name) {
                    return Err(Fail(INVALID_ARGS, format!("no such engine: {name}")));
                }
                self.reset_ctx(bus, path);
                self.attach_engine(path, &name);
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            "GetEngine" => {
                let name = self
                    .ctxs
                    .get(path)
                    .map(|c| c.engine.clone())
                    .unwrap_or_default();
                let mut w = Writer::new();
                w.signature(DESC_SIG);
                write_desc(&mut w, engine_info(&name));
                bus.reply(IBUS_NAME, m, "v", &w.buf);
            }
            "SetSurroundingText" => {
                let text = r.read_ibus_text()?;
                let cursor = r.u32().unwrap_or(0);
                let anchor = r.u32().unwrap_or(0);
                let mut outs = Vec::new();
                if let Some(ime) = self.ctxs.get_mut(path).and_then(|c| c.ime.as_mut()) {
                    ime.set_surrounding_text(&text, cursor, anchor, &mut outs);
                }
                self.emit(bus, path, outs);
                bus.reply(IBUS_NAME, m, "", &[]);
            }
            _ => return Err(Fail(UNKNOWN_METHOD, member.to_string())),
        }
        Ok(())
    }

    // ------------------------------------------------------------ contexts and focus

    fn create_ic(&mut self, owner_unique: &str) -> String {
        self.config.refresh();
        self.next_ic += 1;
        let path = format!("{IC_PREFIX}{}", self.next_ic);
        self.ctxs.insert(
            path.clone(),
            Ctx {
                owner_unique: owner_unique.to_string(),
                engine: String::new(),
                ime: None,
                caps: 0,
                content: (0, 0),
                client_commit: false,
                preedit: None,
            },
        );
        let engine = self.global.clone();
        self.attach_engine(&path, &engine);
        path
    }

    /// Gives a context a fresh engine of the named kind, carrying over its client state.
    fn attach_engine(&mut self, path: &str, engine: &str) {
        let mut ime = self.new_engine(engine);
        if let (Some(ime), Some(c)) = (ime.as_mut(), self.ctxs.get(path)) {
            ime.set_capabilities(c.caps);
            ime.set_content_type(c.content.0, c.content.1);
        }
        if let Some(c) = self.ctxs.get_mut(path) {
            c.engine = engine.to_string();
            c.ime = ime;
            c.preedit = None;
        }
    }

    fn destroy_ic(&mut self, bus: &mut Bus, path: &str) {
        if self.ctxs.remove(path).is_none() {
            return;
        }
        if self.focused.as_deref() == Some(path) {
            self.focused = None;
        }
        bus.call_name(
            PANEL_NAME,
            PANEL_PATH,
            PANEL_IFACE,
            "DestroyContext",
            "o",
            &body_str(path),
        );
    }

    /// Per-app on/off as ibus.rs keeps it: a remembered state for the focused app, else on.
    fn app_enabled(&mut self) -> bool {
        if self.config.current.remember_state_per_app {
            if let Some(app) = self.current_app.clone() {
                return *app_state_mut(&mut self.app_states, &app);
            }
        }
        true
    }

    /// A fresh engine for a context: rustype gets an Ime, anything else is passthrough.
    fn new_engine(&mut self, engine: &str) -> Option<Ime> {
        if engine != ENGINE_RUSTYPE {
            return None;
        }
        let enabled = self.app_enabled();
        let cfg = self.config.current.clone();
        let mut ime = Ime::new(&cfg);
        ime.set_app_id(self.current_app.clone(), &cfg);
        let mut none = Vec::new();
        ime.set_enabled(enabled, &mut none);
        Some(ime)
    }

    fn set_global(&mut self, bus: &mut Bus, name: &str) {
        self.config.refresh();
        self.global = name.to_string();
        let paths: Vec<String> = self.ctxs.keys().cloned().collect();
        for p in paths {
            if self.ctxs.get(&p).is_some_and(|c| c.engine == name) {
                continue;
            }
            self.reset_ctx(bus, &p);
            self.attach_engine(&p, name);
        }
        bus.broadcast_signal(
            IBUS_NAME,
            IBUS_PATH,
            IBUS_IFACE,
            "GlobalEngineChanged",
            "s",
            &body_str(name),
        );
        // The focused context switches engine now, so it announces the new properties too.
        if let Some(p) = self.focused.clone() {
            self.focus_in(bus, &p);
        }
    }

    fn focus_in(&mut self, bus: &mut Bus, path: &str) {
        if let Some(prev) = self.focused.clone() {
            if prev != path {
                self.focus_out(bus, &prev);
            }
        }
        if !self.ctxs.contains_key(path) {
            return;
        }
        self.config.refresh();
        let enabled = self.app_enabled();
        let cfg = self.config.current.clone();
        let app = self.current_app.clone();
        self.focused = Some(path.to_string());

        let mut outs = Vec::new();
        let mut has_ime = false;
        if let Some(ime) = self.ctxs.get_mut(path).and_then(|c| c.ime.as_mut()) {
            ime.focus_in();
            ime.reset(&mut outs);
            ime.set_app_id(app, &cfg);
            ime.set_enabled(enabled, &mut outs);
            has_ime = true;
        }
        self.emit(bus, path, outs);
        if has_ime {
            let mut pw = Writer::new();
            write_prop_list(&mut pw, enabled, cfg.telex.input_method);
            self.ic_and_panel(bus, path, "RegisterProperties", &pw.buf);
            self.ic_and_panel(bus, path, "UpdateProperty", &input_mode_body(enabled));
        }
        bus.call_name(
            PANEL_NAME,
            PANEL_PATH,
            PANEL_IFACE,
            "FocusIn",
            "o",
            &body_str(path),
        );
    }

    fn focus_out(&mut self, bus: &mut Bus, path: &str) {
        if self.focused.as_deref() == Some(path) {
            self.focused = None;
        }
        self.reset_ctx(bus, path);
        bus.call_name(
            PANEL_NAME,
            PANEL_PATH,
            PANEL_IFACE,
            "FocusOut",
            "o",
            &body_str(path),
        );
    }

    /// Reset and focus-out (FACTS B10). A COMMIT-mode preedit still showing is committed by us
    /// exactly once, unless the client does its own commit (ClientCommitPreedit).
    fn reset_ctx(&mut self, bus: &mut Bus, path: &str) {
        let mut outs = Vec::new();
        let Some(ctx) = self.ctxs.get_mut(path) else {
            return;
        };
        if !ctx.client_commit {
            if let Some(text) = ctx.preedit.take() {
                outs.push(Out::Commit(text));
            }
        }
        if let Some(ime) = ctx.ime.as_mut() {
            ime.reset(&mut outs);
        }
        self.emit(bus, path, outs);
    }

    // ------------------------------------------------------------ keys and properties

    fn process_key(
        &mut self,
        bus: &mut Bus,
        path: &str,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> bool {
        if self.focused.as_deref() != Some(path) {
            return false;
        }
        let cfg = &self.config.current;
        let Some(ctx) = self.ctxs.get_mut(path) else {
            return false;
        };
        let Some(ime) = ctx.ime.as_mut() else {
            return false;
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut o = Vec::new();
            let h = ime.process_key(keyval, keycode, state, &mut o);
            (h, o)
        }));
        let (handled, outs) = match result {
            Ok(v) => v,
            Err(_) => {
                // Never take typing down: start the word over and let the app have the key.
                ime.recover(cfg);
                (false, vec![Out::HidePreedit])
            }
        };
        if self.debug {
            eprintln!("rustype-linux: key {keyval:#x} state {state:#x} -> {handled}");
        }
        self.emit(bus, path, outs);
        handled
    }

    /// Menu and client property clicks: the same behaviour as ibus.rs's PropertyActivate.
    fn activate(&mut self, bus: &mut Bus, path: &str, name: &str, state: u32) {
        match name {
            "InputMode" => {
                let Some(cur) = self
                    .ctxs
                    .get(path)
                    .and_then(|c| c.ime.as_ref())
                    .map(|ime| ime.is_enabled())
                else {
                    return;
                };
                let new_state = if state == PROP_STATE_CHECKED {
                    true
                } else if state == PROP_STATE_UNCHECKED {
                    false
                } else {
                    !cur
                };
                if let Some(app) = self.current_app.clone() {
                    *app_state_mut(&mut self.app_states, &app) = new_state;
                }
                let mut outs = Vec::new();
                if let Some(ime) = self.ctxs.get_mut(path).and_then(|c| c.ime.as_mut()) {
                    ime.set_enabled(new_state, &mut outs);
                }
                self.emit(bus, path, outs);
                self.ic_and_panel(bus, path, "UpdateProperty", &input_mode_body(new_state));
                bus.call_name(PANEL_NAME, PANEL_PATH, PANEL_IFACE, "StateChanged", "", &[]);
            }
            "prop_telex" | "prop_vni" => {
                if state != PROP_STATE_CHECKED {
                    return;
                }
                let method = if name == "prop_telex" {
                    InputMethod::Telex
                } else {
                    InputMethod::Vni
                };
                self.config.current.telex.input_method = method;
                update_input_method(method);
                let cfg = self.config.current.clone();
                for ctx in self.ctxs.values_mut() {
                    if let Some(ime) = ctx.ime.as_mut() {
                        ime.apply_config(&cfg);
                    }
                }
                let telex = method == InputMethod::Telex;
                self.ic_and_panel(
                    bus,
                    path,
                    "UpdateProperty",
                    &radio_body("prop_telex", "Kiểu gõ Telex", telex),
                );
                self.ic_and_panel(
                    bus,
                    path,
                    "UpdateProperty",
                    &radio_body("prop_vni", "Kiểu gõ VNI", !telex),
                );
            }
            "prop_settings" => {
                let home = std::env::var("HOME").unwrap_or_default();
                let candidates = [
                    "/usr/bin/rustype-settings".to_string(),
                    format!("{home}/.local/bin/rustype-settings"),
                ];
                let mut spawned = false;
                for exe in &candidates {
                    if std::path::Path::new(exe).exists() && spawn_and_reap(exe) {
                        spawned = true;
                        break;
                    }
                }
                if !spawned {
                    spawn_and_reap("rustype-settings");
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ output

    /// Sends one engine output to the context's client, in order, as IBus's input-context
    /// signals. Also keeps the COMMIT-mode preedit that reset/focus-out may need to commit.
    fn emit(&mut self, bus: &mut Bus, path: &str, outs: Vec<Out>) {
        if outs.is_empty() {
            return;
        }
        let Some(ctx) = self.ctxs.get_mut(path) else {
            return;
        };
        let owner = ctx.owner_unique.clone();
        for o in outs {
            let bytes = match o {
                Out::Commit(text) => {
                    let mut w = Writer::new();
                    write_text(&mut w, &text, false);
                    bus.ibus_signal(&owner, path, IC_IFACE, "CommitText", "v", &w.buf)
                }
                Out::Preedit(text) => {
                    ctx.preedit = (!text.is_empty()).then(|| text.clone());
                    let mut w = Writer::new();
                    write_text(&mut w, &text, true);
                    w.u32(text.chars().count() as u32);
                    w.boolean(true);
                    w.u32(MODE_COMMIT);
                    bus.ibus_signal(
                        &owner,
                        path,
                        IC_IFACE,
                        "UpdatePreeditTextWithMode",
                        "vubu",
                        &w.buf,
                    )
                }
                Out::HidePreedit => {
                    ctx.preedit = None;
                    bus.ibus_signal(&owner, path, IC_IFACE, "HidePreeditText", "", &[])
                }
                Out::DeleteSurrounding { offset, nchars } => {
                    let mut w = Writer::new();
                    w.i32(offset);
                    w.u32(nchars);
                    bus.ibus_signal(
                        &owner,
                        path,
                        IC_IFACE,
                        "DeleteSurroundingText",
                        "iu",
                        &w.buf,
                    )
                }
                Out::ForwardKey {
                    keyval,
                    keycode,
                    state,
                } => {
                    let mut w = Writer::new();
                    w.u32(keyval);
                    w.u32(keycode);
                    w.u32(state);
                    bus.ibus_signal(&owner, path, IC_IFACE, "ForwardKeyEvent", "uuu", &w.buf)
                }
                Out::RequireSurrounding => {
                    bus.ibus_signal(&owner, path, IC_IFACE, "RequireSurroundingText", "", &[])
                }
            };
            bus.queue_unique(&owner, &bytes);
        }
        if self.debug {
            eprintln!("rustype-linux: emitted to {path}");
        }
    }

    /// RegisterProperties / UpdateProperty go to the client's context and to the panel.
    fn ic_and_panel(&mut self, bus: &mut Bus, path: &str, member: &str, body: &[u8]) {
        let Some(owner) = self.ctxs.get(path).map(|c| c.owner_unique.clone()) else {
            return;
        };
        let bytes = bus.ibus_signal(&owner, path, IC_IFACE, member, "v", body);
        bus.queue_unique(&owner, &bytes);
        bus.call_name(PANEL_NAME, PANEL_PATH, PANEL_IFACE, member, "v", body);
    }
}

/// `as` argument read from a body.
fn read_string_array(r: &mut Reader) -> io::Result<Vec<String>> {
    let len = r.u32()? as usize;
    r.align(4);
    let end = r.pos + len;
    let mut out = Vec::new();
    while r.pos < end {
        out.push(r.string()?);
    }
    Ok(out)
}

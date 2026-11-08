//! IBus wire helpers shared by the service: IBusText, IBusProperty and IBusPropList variants,
//! the remembered on/off state per app, and the launcher of the settings app.

use crate::dbus::Writer;
use rustype_core::telex::InputMethod;
use std::collections::HashMap;

pub const RUSTYPE_IFACE: &str = "org.freedesktop.IBus.Rustype";
const ATTR_UNDERLINE: u32 = 1;
const ATTR_UNDERLINE_SINGLE: u32 = 1;

pub const PROP_TYPE_NORMAL: u32 = 0;
pub const PROP_TYPE_TOGGLE: u32 = 1;
pub const PROP_TYPE_RADIO: u32 = 2;
pub const PROP_TYPE_SEPARATOR: u32 = 4;

pub const PROP_STATE_UNCHECKED: u32 = 0;
pub const PROP_STATE_CHECKED: u32 = 1;

/// Writes an IBusText as a variant: v -> (sa{sv}sv) with an IBusAttrList (sa{sv}av) inside.
pub fn write_text(w: &mut Writer, text: &str, underline: bool) {
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

pub fn write_prop(
    w: &mut Writer,
    key: &str,
    ptype: u32,
    label: &str,
    icon: &str,
    state: u32,
    symbol: &str,
) {
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
    // symbol: field 12
    write_text(w, symbol, false);
}

pub fn write_prop_list(w: &mut Writer, enabled: bool, method: InputMethod) {
    w.signature("(sa{sv}av)");
    w.struct_start();
    w.string("IBusPropList");
    let a = w.array_start(8);
    w.array_end(a);
    let props = w.array_start(1);

    // 1. Toggle switch & Indicator symbol: InputMode
    let toggle_state = if enabled {
        PROP_STATE_CHECKED
    } else {
        PROP_STATE_UNCHECKED
    };
    let sym = if enabled { "VN" } else { "EN" };
    write_prop(
        w,
        "InputMode",
        PROP_TYPE_TOGGLE,
        "Bật tiếng Việt",
        "",
        toggle_state,
        sym,
    );

    // 2. Separator
    write_prop(w, "prop_sep1", PROP_TYPE_SEPARATOR, "", "", 0, "");

    // 3. Radio Telex
    let telex_state = if method == InputMethod::Telex {
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
        "",
    );

    // 4. Radio VNI
    let vni_state = if method == InputMethod::Vni {
        PROP_STATE_CHECKED
    } else {
        PROP_STATE_UNCHECKED
    };
    write_prop(
        w,
        "prop_vni",
        PROP_TYPE_RADIO,
        "Kiểu gõ VNI",
        "",
        vni_state,
        "",
    );

    // 5. Separator
    write_prop(w, "prop_sep2", PROP_TYPE_SEPARATOR, "", "", 0, "");

    // 6. Settings
    write_prop(
        w,
        "prop_settings",
        PROP_TYPE_NORMAL,
        "Cài đặt Rustype...",
        "preferences-system",
        0,
        "",
    );

    w.array_end(props);
}

/// Upper bound on remembered per-app states; app ids come from clients, so the map must not grow forever.
const MAX_APP_STATES: usize = 256;

/// The remembered on/off state for `id`, inserting `true` for a new app. When the map is full,
/// an arbitrary existing entry is dropped first.
pub fn app_state_mut<'a>(states: &'a mut HashMap<String, bool>, id: &str) -> &'a mut bool {
    if !states.contains_key(id) && states.len() >= MAX_APP_STATES {
        if let Some(old) = states.keys().next().cloned() {
            states.remove(&old);
        }
    }
    states.entry(id.to_string()).or_insert(true)
}

/// Runs a program without waiting for it; a thread reaps it so it never becomes a zombie.
pub fn spawn_and_reap(exe: &str) -> bool {
    match std::process::Command::new(exe).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus::{encode, read_message, Field, Reader, SIGNAL};

    const ENGINE_IFACE: &str = "org.freedesktop.IBus.Engine";

    /// The body of a signal with this interface member, parsed back: returns the message.
    fn signal_round_trip(member: &str, body: &[u8]) -> crate::dbus::Message {
        let fields = vec![
            Field::Path("/org/freedesktop/IBus/Engine/1"),
            Field::Interface(ENGINE_IFACE),
            Field::Member(member),
        ];
        let blob = encode(SIGNAL, 0, 1, &fields, "v", body);
        read_message(&mut &blob[..]).unwrap()
    }

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
        write_prop_list(&mut w, true, InputMethod::Telex);
        let end = w.buf.len();
        let mut r = Reader::new(&w.buf, false);
        let mut i = 0;
        r.skip(b"v", &mut i).unwrap();
        assert_eq!(r.pos, end);

        let msg = signal_round_trip("RegisterProperties", &w.buf);
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
            "InputMode",
            PROP_TYPE_TOGGLE,
            "Bật tiếng Việt",
            "",
            PROP_STATE_CHECKED,
            "VN",
        );
        let end = w.buf.len();
        let mut r = Reader::new(&w.buf, false);
        let mut i = 0;
        r.skip(b"v", &mut i).unwrap();
        assert_eq!(r.pos, end);

        let msg = signal_round_trip("UpdateProperty", &w.buf);
        assert_eq!(msg.member.as_deref(), Some("UpdateProperty"));
        assert_eq!(msg.signature, "v");
    }

    #[test]
    fn app_states_stay_bounded() {
        let mut states = HashMap::new();
        for i in 0..(MAX_APP_STATES * 3) {
            let id = format!("app-{i}");
            *app_state_mut(&mut states, &id) = false;
            assert!(states.len() <= MAX_APP_STATES);
        }
        assert_eq!(states.len(), MAX_APP_STATES);
        // A known app keeps its stored value and is not evicted by its own lookup.
        let last = format!("app-{}", MAX_APP_STATES * 3 - 1);
        assert!(!*app_state_mut(&mut states, &last));
        assert_eq!(states.len(), MAX_APP_STATES);
    }
}

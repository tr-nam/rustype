"""Panel side of rustype-linux, run inside tools/rustype_linux_sandbox.sh on a private bus.

A real libibus PanelService owns org.freedesktop.IBus.Panel. Checks: focus-in and
register-properties arrive when a context takes focus; panel.property_activate on InputMode
turns conversion off (keys pass through) and back on (keys convert again)."""
import sys, time
import gi
gi.require_version("IBus", "1.0")
from gi.repository import IBus, GLib

IBus.init()
bus = IBus.Bus()
for _ in range(50):
    if bus.is_connected():
        break
    time.sleep(0.1)
    bus = IBus.Bus()
if not bus.is_connected():
    print("FAIL could not connect to rustype-linux")
    sys.exit(1)

ctx = GLib.MainContext.default()
def pump(t=0.05):
    end = time.time() + t
    while time.time() < end:
        while ctx.iteration(False):
            pass
        time.sleep(0.002)

results = []
def check(name, ok, detail=""):
    print(("OK   " if ok else "FAIL ") + name + ("" if ok else f" {detail}"))
    results.append(ok)

events = {"focus-in": [], "focus-out": [], "register-properties": [], "update-property": []}
reply = bus.request_name("org.freedesktop.IBus.Panel", IBus.BusNameFlag.REPLACE_EXISTING)
check("request org.freedesktop.IBus.Panel is primary owner", reply == 1, f"reply {reply}")

panel = IBus.PanelService.new(bus.get_connection())
panel.connect("focus-in", lambda p, path, *rest: events["focus-in"].append(path))
panel.connect("focus-out", lambda p, path, *rest: events["focus-out"].append(path))
panel.connect("register-properties", lambda p, props: events["register-properties"].append(props))
panel.connect("update-property", lambda p, prop: events["update-property"].append(prop))

bus.set_global_engine("rustype")
pump(0.3)

shown = []
def on_preedit(c, text, pos, visible, mode):
    if visible:
        shown.append(text.get_text())
ic = bus.create_input_context("panel-test")
ic.connect("update-preedit-text-with-mode", on_preedit)
ic.set_capabilities(IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.FOCUS)
ic.set_client_commit_preedit(False)
ic.focus_in()
pump(0.4)

path = ic.get_object_path()
check("focus-in reaches the panel for this context", path in events["focus-in"], events["focus-in"])
check("register-properties reaches the panel", len(events["register-properties"]) > 0)

def type_keys(keys):
    handled = []
    for ch in keys:
        handled.append(ic.process_key_event(ord(ch), 0, 0))
        ic.process_key_event(ord(ch), 0, IBus.ModifierType.RELEASE_MASK)
        pump(0.02)
    pump(0.1)
    return handled

shown.clear()
h = type_keys("aa")
check("VN on: 'aa' is converted (keys handled)", all(h), h)
check("VN on: preedit shows the converted letter", bool(shown) and shown[-1] == "â", shown)

panel.property_activate("InputMode", 0)
pump(0.4)
shown.clear()
h = type_keys("aa")
check("InputMode off via panel: keys pass through", not any(h), h)
check("InputMode off via panel: no preedit", shown == [], shown)

panel.property_activate("InputMode", 1)
pump(0.4)
shown.clear()
h = type_keys("aa")
check("InputMode on via panel: conversion resumes", all(h) and bool(shown) and shown[-1] == "â", (h, shown))

ic.focus_out()
pump(0.3)
check("focus-out reaches the panel", path in events["focus-out"], events["focus-out"])
ic.destroy()
pump(0.1)

# rustype's own interface (what the GNOME extension calls): per-app on/off state.
def rustype_call(member, sig=None, args=None):
    v = bus.get_connection().call_sync(
        "org.freedesktop.IBus.Rustype", "/org/freedesktop/IBus/Factory",
        "org.freedesktop.IBus.Rustype", member,
        GLib.Variant(sig, args) if sig else None, None, 0, 5000, None)
    return v.unpack()[0] if v.n_children() else None
rustype_call("SetFocusApp", "(s)", ("org.test.App",))
state0 = rustype_call("GetState")
rustype_call("ToggleState")
state1 = rustype_call("GetState")
rustype_call("ToggleState")
state2 = rustype_call("GetState")
check("Rustype.ToggleState flips the per-app state", state0 is True and state1 is False and state2 is True,
      (state0, state1, state2))
rustype_call("SetFocusApp", "(s)", ("org.test.Other",))
check("Rustype.SetFocusApp: an unseen app starts enabled", rustype_call("GetState") is True)
print("PASS" if all(results) else "SOME FAILED")
sys.exit(0 if all(results) else 1)

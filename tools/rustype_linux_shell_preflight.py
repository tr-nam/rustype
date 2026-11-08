"""Stand-in for GNOME Shell's IBus client, run inside tools/rustype_linux_sandbox.sh.

Reproduces the call sequence of ibusManager.js and inputMethod.js on real libibus (async variants,
two Bus objects, PanelService, input context signals), then tests reconnection: a server is stopped,
a NEW server on another socket takes over the address file through rename, and the whole
sequence must work again. Run with G_DEBUG=fatal-criticals so any GLib CRITICAL aborts.

usage: rustype_linux_shell_preflight.py RUSTYPE_LINUX_BINARY WORKDIR   (IBUS_ADDRESS_FILE must be set)
"""
import os, subprocess, sys, time
import gi
gi.require_version("IBus", "1.0")
from gi.repository import IBus, GLib, GObject

BIN, WORK = sys.argv[1], sys.argv[2]
ADDR_FILE = os.environ["IBUS_ADDRESS_FILE"]
os.environ.pop("IBUS_ADDRESS", None)
ctx_main = GLib.MainContext.default()
results = []
# Signal arguments such as IBus.Text and IBus.PropList arrive floating. PyGObject unrefs them when
# the Python wrapper is dropped, which logs g_object_is_floating CRITICALs (fatal under
# G_DEBUG=fatal-criticals). Keeping every GObject argument alive avoids that; the list lives as long
# as the test.
KEEP = []

def on(obj, signal, fn):
    def handler(*args):
        KEEP.extend(a for a in args if isinstance(a, GObject.Object))
        return fn(*args)
    obj.connect(signal, handler)

def check(name, ok, detail=""):
    print(("OK   " if ok else "FAIL ") + name + ("" if ok else f" {detail}"), flush=True)
    results.append(bool(ok))

def pump(t=0.05):
    end = time.time() + t
    while time.time() < end:
        while ctx_main.iteration(False):
            pass
        time.sleep(0.002)

def wait_until(cond, timeout, what):
    end = time.time() + timeout
    while time.time() < end:
        if cond():
            return True
        pump(0.05)
    check(what, False, "timed out")
    return False

def run_async(start, finish, *args, timeout=10):
    """Calls an async libibus function and returns its result (pumping the main loop)."""
    out = {}
    def cb(src, res, _data):
        try:
            out["v"] = getattr(src, finish)(res)
        except Exception as e:  # noqa: BLE001 - reported below
            out["e"] = e
    start(*args, None, cb, None)
    wait_until(lambda: "v" in out or "e" in out, timeout, f"{finish} completes")
    if "e" in out:
        raise out["e"]
    return out.get("v")

class Editor:
    """What a text field does: keeps text, applies commits, deletions and preedit like GNOME Shell."""
    def __init__(self):
        self.text, self.preedit, self.mode, self.shown, self.deletes, self.requires = "", "", 0, [], 0, 0
        self.fwd = []
        # Omnibox model (table set): the inline suggestion selected after the caret.
        self.table, self.sugg = None, ""
    def forward(self, kv, st):
        self.fwd.append((kv, st))
    def insert(self, ch):
        self.sugg = ""  # typed text replaces the selected suggestion
        self.text += ch
    def omni_suggest(self):
        if self.table is not None:
            self.sugg = self.table.get(self.text, "")
    def commit(self, s):
        self.sugg = ""  # a commit replaces the selected suggestion
        self.text += s
    def preedit_update(self, s, visible, mode):
        self.preedit = s if visible else ""
        if visible and s:
            self.shown.append(s)
        if visible and mode is not None:
            self.mode = mode
    def delete(self, n):
        self.deletes += 1
        if self.sugg:
            return  # Chrome drops a delete while a suggestion is selected
        self.text = self.text[: len(self.text) - n]
    def click_or_focus_out(self):
        # Shell (client commit) commits a COMMIT-mode preedit itself when focus or the cursor moves.
        if self.preedit and self.mode == 1:
            self.text += self.preedit
        self.preedit = ""

TRAN = "Trần"  # "Trầ" + "n" from Telex "Traanf"

def sequence(bus_m, bus_i, tag):
    """ibusManager.js and inputMethod.js, in the order Shell uses them."""
    global panel
    got = {"global": [], "focus-in": [], "focus-out": [], "register": [], "update": [],
           "cursor": [], "content": []}
    bus_m.set_watch_ibus_signal(True)
    on(bus_m, "global-engine-changed", lambda b, name: got["global"].append(name))
    engines = run_async(bus_m.list_engines_async, "list_engines_async_finish", -1)
    names = [e.get_name() for e in engines]
    check(f"{tag}: ListEngines offers rustype and xkb:us::eng", "rustype" in names and "xkb:us::eng" in names, names)
    reply = run_async(bus_m.request_name_async, "request_name_async_finish", IBus.SERVICE_PANEL,
                      IBus.BusNameFlag.ALLOW_REPLACEMENT | IBus.BusNameFlag.REPLACE_EXISTING, -1)
    check(f"{tag}: request org.freedesktop.IBus.Panel (REPLACE_EXISTING)", reply == 1, reply)
    panel = IBus.PanelService(connection=bus_m.get_connection(), object_path=IBus.PATH_PANEL)
    on(panel, "update-property", lambda p, prop: got["update"].append(prop))
    on(panel, "set-cursor-location", lambda p, x, y, w, h: got["cursor"].append((x, y, w, h)))
    on(panel, "focus-in", lambda p, path, *rest: got["focus-in"].append(path))
    on(panel, "focus-out", lambda p, path, *rest: got["focus-out"].append(path))
    on(panel, "set-content-type", lambda p, purpose, hints: got["content"].append(purpose))
    on(panel, "register-properties", lambda p, props: got["register"].append(props))
    cur = run_async(bus_m.get_global_engine_async, "get_global_engine_async_finish", -1)
    check(f"{tag}: GetGlobalEngine returns an engine", cur is not None)
    run_async(bus_m.preload_engines_async, "preload_engines_async_finish", ["rustype", "xkb:us::eng"], -1)
    run_async(bus_m.set_global_engine_async, "set_global_engine_async_finish", "rustype", -1)
    wait_until(lambda: "rustype" in got["global"], 5, f"{tag}: global-engine-changed(rustype)")
    check(f"{tag}: global-engine-changed(rustype) received", "rustype" in got["global"], got["global"])

    ctx = run_async(bus_i.create_input_context_async, "create_input_context_async_finish", "gnome-shell", -1)
    ed = Editor()
    got_require = []
    on(ctx, "commit-text", lambda c, t: ed.commit(t.get_text()))
    on(ctx, "update-preedit-text-with-mode", lambda c, t, p, vis, m: ed.preedit_update(t.get_text(), vis, m))
    on(ctx, "hide-preedit-text", lambda c: ed.preedit_update("", False, None))
    on(ctx, "show-preedit-text", lambda c: None)
    on(ctx, "delete-surrounding-text", lambda c, off, n: ed.delete(n))
    on(ctx, "forward-key-event", lambda c, kv, kc, st: ed.forward(kv, st))
    on(ctx, "require-surrounding-text", lambda c: got_require.append(1))
    on(ctx, "destroy", lambda c: None)
    ctx.set_client_commit_preedit(True)
    caps = IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.FOCUS | IBus.Capabilite.SURROUNDING_TEXT
    ctx.set_capabilities(caps)
    ctx.focus_in()
    pump(0.3)
    check(f"{tag}: focus-in reaches the panel", ctx.get_object_path() in got["focus-in"], got["focus-in"])
    check(f"{tag}: register-properties reaches the panel", len(got["register"]) > 0)

    def report():
        # Chrome reports Surrounding Text only once the engine asked for it.
        if ed.table is not None and ctx.needs_surrounding_text():
            n = len(ed.text)
            ctx.set_surrounding_text(IBus.Text.new_from_string(ed.text + ed.sugg), n, n + len(ed.sugg))

    def keys(text, lag=False):
        """lag: all keys first, then one report, as if Chrome's reports arrive late."""
        handled = []
        for ch in text:
            h = run_async(ctx.process_key_event_async, "process_key_event_async_finish", ord(ch), 0, 0, -1)
            run_async(ctx.process_key_event_async, "process_key_event_async_finish", ord(ch), 0,
                      IBus.ModifierType.RELEASE_MASK, -1)
            handled.append(h)
            if not h:
                ed.insert(ch)
            ed.omni_suggest()
            if not lag:
                report()
            pump(0.02)
        if lag:
            report()
        pump(0.3 if lag else 0.1)
        return handled

    # Purpose URL (Omnibox): native keys, no preedit, no forwarded keys. Surrounding Text is
    # reported once the engine asks for it, and a delete Chrome drops under a suggestion is repaired.
    OMNI = {"Ru": "st-lang.org", "Tra": "nslate.google.com"}
    ed.text, ed.preedit, ed.sugg, ed.table = "", "", "", OMNI
    ctx.set_content_type(5, 0)
    ctx.set_cursor_location(0, 0, 0, 0)
    ctx.set_surrounding_text(IBus.Text.new_from_string(""), 0, 0)
    pump(0.3)
    shown_before, fwd_before = len(ed.shown), len(ed.fwd)
    keys("Rus ")
    check(f"{tag}: purpose URL: Rus gives Rú", ed.text == "Rú ", repr(ed.text))
    check(f"{tag}: purpose URL: no preedit", ed.shown[shown_before:] == [], ed.shown[shown_before:])
    check(f"{tag}: purpose URL: no forward-key", ed.fwd[fwd_before:] == [], ed.fwd[fwd_before:])
    check(f"{tag}: RequireSurroundingText sent in URL fields", len(got_require) > 0, len(got_require))
    ed.text, ed.sugg = "", ""
    ctx.reset()  # a new, empty field: the engine must forget the first word
    ctx.set_surrounding_text(IBus.Text.new_from_string(""), 0, 0)
    pump(0.3)
    keys("Traanf ", lag=True)
    check(f"{tag}: purpose URL: Traanf gives Trần", ed.text == "Trần ", repr(ed.text))
    check(f"{tag}: purpose URL 2: no preedit", ed.shown[shown_before:] == [], ed.shown[shown_before:])
    check(f"{tag}: purpose URL 2: no forward-key", ed.fwd[fwd_before:] == [], ed.fwd[fwd_before:])
    ed.table, ed.sugg = None, ""
    ctx.reset()
    pump(0.3)

    # Purpose 0 (normal field): Surrounding Text path, text synced from the editor.
    ed.text, ed.preedit = "", ""
    ctx.set_content_type(0, 0)
    ctx.set_surrounding_text(IBus.Text.new_from_string(""), 0, 0)
    pump(0.3)
    h = keys("Traanf")
    check(f"{tag}: purpose 0: word is in the field", ed.text == TRAN, repr(ed.text))

    # Passthrough engine (xkb:us): keys must pass through.
    run_async(bus_m.set_global_engine_async, "set_global_engine_async_finish", "xkb:us::eng", -1)
    wait_until(lambda: "xkb:us::eng" in got["global"], 5, f"{tag}: global engine -> xkb")
    pump(0.3)
    h = keys("aa")
    check(f"{tag}: xkb engine: keys pass through", not any(h), h)
    run_async(bus_m.set_global_engine_async, "set_global_engine_async_finish", "rustype", -1)
    pump(0.3)
    ed.text = ""
    ed.text = ""
    h = keys("aa")
    check(f"{tag}: back on rustype: aa becomes â", ed.text == "\u00e2", (h, repr(ed.text)))

    # Menu: InputMode off, then on, through the panel.
    panel.property_activate("InputMode", 0)
    pump(0.4)
    h = keys("aa")
    check(f"{tag}: panel InputMode off: keys pass through", not any(h), h)
    panel.property_activate("InputMode", 1)
    pump(0.4)
    ed.text = ""
    h = keys("aa")
    check(f"{tag}: panel InputMode on: aa becomes â", ed.text == "\u00e2", (h, repr(ed.text)))
    check(f"{tag}: content-type reached the panel", 5 in got["content"] and 0 in got["content"], got["content"])
    check(f"{tag}: cursor location reached the panel", len(got["cursor"]) >= 1)
    ctx.reset()
    ctx.focus_out()
    pump(0.3)
    check(f"{tag}: focus-out reaches the panel", ctx.get_object_path() in got["focus-out"], got["focus-out"])
    ctx.destroy()
    pump(0.1)

def start_server(sock):
    env = dict(os.environ, IBUS_ADDRESS_FILE=ADDR_FILE)
    p = subprocess.Popen([BIN, "--address", f"unix:path={sock}", "--own-session-name"], env=env,
                         stdout=subprocess.DEVNULL, stderr=open(os.path.join(WORK, f"{os.path.basename(sock)}.log"), "w"))
    return p

def wait_file_for(proc_pid):
    end = time.time() + 10
    while time.time() < end:
        if os.path.exists(ADDR_FILE) and f"IBUS_DAEMON_PID={proc_pid}" in open(ADDR_FILE).read():
            return True
        time.sleep(0.05)
    return False

def main():
    s1 = start_server(os.path.join(WORK, "p1.sock"))
    check("preflight server 1 wrote the address file", wait_file_for(s1.pid))
    bus_m = IBus.Bus.new_async()
    bus_i = IBus.Bus.new_async()
    state = {"conn_m": 0, "disc_m": 0, "conn_i": 0, "disc_i": 0}
    on(bus_m, "connected", lambda b: state.__setitem__("conn_m", state["conn_m"] + 1))
    on(bus_m, "disconnected", lambda b: state.__setitem__("disc_m", state["disc_m"] + 1))
    on(bus_i, "connected", lambda b: state.__setitem__("conn_i", state["conn_i"] + 1))
    on(bus_i, "disconnected", lambda b: state.__setitem__("disc_i", state["disc_i"] + 1))
    wait_until(lambda: bus_m.is_connected() and bus_i.is_connected(), 10, "both Bus objects connected")
    sequence(bus_m, bus_i, "first")

    # Takeover: the old daemon stops, a new one (new socket) writes the address file.
    s1.terminate()
    s1.wait(timeout=10)
    wait_until(lambda: state["disc_m"] >= 1 and state["disc_i"] >= 1, 10, "both Bus objects report disconnected")
    check("disconnected after the server stopped", state["disc_m"] >= 1 and state["disc_i"] >= 1, state)
    s2 = start_server(os.path.join(WORK, "p2.sock"))
    reconnected = wait_until(lambda: bus_m.is_connected() and bus_i.is_connected(), 20,
                             "libibus reconnects to the new server")
    check("libibus reconnects (both Bus objects connected again)", bool(reconnected) and state["conn_m"] >= 2 and state["conn_i"] >= 2, state)
    if reconnected:
        sequence(bus_m, bus_i, "after reconnect")
    s2.terminate()
    s2.wait(timeout=10)
    print("PASS" if all(results) else "SOME FAILED", flush=True)
    sys.exit(0 if all(results) else 1)

if __name__ == "__main__":
    main()

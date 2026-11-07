"""Fake apps talking to a private ibus-daemon: checks what text actually lands in each app.
Run through tools/ibus_sandbox_test.sh, never against the real session."""
import os, sys, time, gi
gi.require_version("IBus", "1.0")
from gi.repository import IBus, GLib

IBus.init()
bus = IBus.Bus()
for _ in range(50):
    if bus.is_connected():
        break
    time.sleep(0.1)
    bus = IBus.Bus()
ENGINE = sys.argv[1] if len(sys.argv) > 1 else "rustype"
ctx = GLib.MainContext.default()

def pump(t=0.05):
    end = time.time() + t
    while time.time() < end:
        while ctx.iteration(False):
            pass
        time.sleep(0.002)

class App:
    """A text field. surrounding: reports Surrounding Text. client_commit: commits a COMMIT-mode
    preedit itself on reset/focus-out, like GNOME Shell; otherwise ibus-daemon does it (X11 apps)."""
    def __init__(self, name, surrounding, client_commit):
        self.name, self.text, self.preedit, self.mode, self.modes = name, "", "", 0, set()
        self.client_commit, self.latency = client_commit, []
        ic = self.ic = bus.create_input_context(name)
        ic.connect("commit-text", lambda c, t: self._commit(t.get_text()))
        ic.connect("update-preedit-text-with-mode", lambda c, t, p, vis, m: self._preedit(t.get_text(), vis, m))
        ic.connect("update-preedit-text", lambda c, t, p, vis: self._preedit(t.get_text(), vis, None))
        ic.connect("hide-preedit-text", lambda c: self._preedit("", False, None))
        ic.connect("delete-surrounding-text", lambda c, off, n: self._delete(n))
        caps = IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.FOCUS
        if surrounding:
            caps |= IBus.Capabilite.SURROUNDING_TEXT
        ic.set_capabilities(caps)
        ic.set_client_commit_preedit(client_commit)
        ic.focus_in()
        pump(0.2)
        bus.set_global_engine(ENGINE)
        pump(1.0)
        if surrounding:
            self._sync()

    def _sync(self):
        n = len(self.text)
        self.ic.set_surrounding_text(IBus.Text.new_from_string(self.text), n, n)
    def _commit(self, s):
        self.text += s
    def _preedit(self, s, visible, mode):
        self.preedit = s if visible else ""
        if visible and mode is not None:
            self.mode = mode
            self.modes.add("COMMIT" if mode == 1 else "CLEAR")
    def _delete(self, n):
        self.text = self.text[: len(self.text) - n]

    def type(self, keys):
        for ch in keys:
            kv = {"\b": IBus.KEY_BackSpace, "\n": IBus.KEY_Return}.get(ch, ord(ch))
            t0 = time.perf_counter()
            handled = self.ic.process_key_event(kv, 0, 0)
            self.latency.append(time.perf_counter() - t0)
            self.ic.process_key_event(kv, 0, IBus.ModifierType.RELEASE_MASK)
            pump(0.01)
            if not handled:
                if ch == "\b":
                    self.text = self.text[:-1]
                elif ch == "\n":
                    self.text += "⏎"
                else:
                    self.text += ch
        pump(0.1)

    def click(self):
        """Mouse click inside the field: GNOME commits a COMMIT-mode preedit, then resets."""
        if self.client_commit and self.preedit and self.mode == 1:
            self.text += self.preedit
        self.preedit = ""
        self.ic.reset()
        pump(0.3)

    def focus_out(self):
        if self.client_commit and self.preedit and self.mode == 1:
            self.text += self.preedit
        self.preedit = ""
        self.ic.focus_out()
        pump(0.3)

def check(name, got, want):
    ok = got == want
    print(f"{'OK  ' if ok else 'FAIL'} {name}: {got!r}" + ("" if ok else f" (want {want!r})"))
    return ok

results = []
lat = []
SENT = "Kieemr tra mays tooi hieenj taij toanf booj gox vaof terminal cuar banj ddang bij looix "
WANT = "Kiểm tra máy tôi hiện tại toàn bộ gõ vào terminal của bạn đang bị lỗi "

term = App("terminal", surrounding=False, client_commit=True)
term.type(SENT)
results.append(check("terminal sentence", term.text, WANT))
results.append(check("terminal preedit mode", sorted(term.modes), ["COMMIT"]))
term.type("vieetj")
term.click()
term.type(" nam ")
results.append(check("terminal click keeps word once", term.text[len(WANT):], "việt nam "))
term.type("ddang gox\n")
results.append(check("terminal enter", term.text[len(WANT) + 9:], "đang gõ⏎"))
term.type("hoafn\b\b ")
results.append(check("terminal backspace in preedit", term.text[-3:], "hò "))
term.type("tieengs")
term.focus_out()
results.append(check("terminal focus-out keeps word once", term.text[-5:], "tiếng"))
lat += term.latency
term.ic.destroy()

gui = App("gui", surrounding=True, client_commit=True)
gui.type(SENT)
results.append(check("gui app sentence (no underline)", gui.text, WANT))
results.append(check("gui app never shows preedit", sorted(gui.modes), []))
gui.type("hoafn\b ")
results.append(check("gui backspace re-places tone", gui.text[-4:], "hòa "))
lat += gui.latency
gui.ic.destroy()

x11 = App("x11", surrounding=False, client_commit=False)
x11.type("nguowif ")
x11.type("dduwowcj")
x11.click()
results.append(check("x11 app: daemon commits preedit once on click", x11.text, "người được"))
lat += x11.latency
x11.ic.destroy()

lat.sort()
print(f"key round trip (client -> ibus-daemon -> engine -> client): median {lat[len(lat)//2]*1e6:.0f} us, p95 {lat[int(len(lat)*0.95)]*1e6:.0f} us, n={len(lat)}")
print("ALL PASS" if all(results) else "SOME FAILED")

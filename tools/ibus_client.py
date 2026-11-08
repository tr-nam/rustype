"""Fake apps talking to a private rustype-linux bus: checks what text actually lands in each app.
Run through tools/rustype_linux_sandbox.sh, never against the real session."""
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
    preedit itself on reset/focus-out, like GNOME Shell; otherwise the IBus bus does it (X11 apps)."""
    def __init__(self, name, surrounding, client_commit, table=None):
        self.name, self.text, self.preedit, self.mode, self.modes = name, "", "", 0, set()
        self.client_commit, self.latency = client_commit, []
        self.shown, self.deletes, self.fwd = [], 0, []
        # Omnibox model (table set): the inline suggestion selected after the caret.
        self.table, self.sugg, self.required = table, "", 0
        ic = self.ic = bus.create_input_context(name)
        ic.connect("commit-text", lambda c, t: self._commit(t.get_text()))
        ic.connect("forward-key-event", lambda c, kv, kc, st: self._forward(kv, st))
        ic.connect("require-surrounding-text", lambda c: self._require())
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
        self.sugg = ""  # a commit replaces the selected suggestion
        self.text += s
    def _require(self):
        self.required += 1
    def _omni_suggest(self):
        """Chrome: a suggestion is selected after the caret as the text changes."""
        if self.table is not None:
            self.sugg = self.table.get(self.text, "")
    def _omni_report(self):
        """Surrounding Text is reported only when the engine asked for it
        (ic.needs_surrounding_text), and Chrome's report can lag behind the keys."""
        if self.table is None:
            return
        if self.ic.needs_surrounding_text():
            n = len(self.text)
            self.ic.set_surrounding_text(IBus.Text.new_from_string(self.text + self.sugg), n, n + len(self.sugg))
    def _preedit(self, s, visible, mode):
        self.preedit = s if visible else ""
        if visible and s:
            self.shown.append(s)
        if visible and mode is not None:
            self.mode = mode
            self.modes.add("COMMIT" if mode == 1 else "CLEAR")
    def _delete(self, n):
        self.deletes += 1
        if self.sugg:
            return  # Chrome drops a delete while a suggestion is selected
        self.text = self.text[: len(self.text) - n]
    def _forward(self, kv, st):
        self.fwd.append((kv, st))

    def type(self, keys, lag=False):
        """lag: all keys first, then one report, as if Chrome's reports arrive late."""
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
                    self.sugg = ""  # typed text replaces the selected suggestion
                    self.text += ch
            self._omni_suggest()
            if not lag:
                self._omni_report()
        if lag:
            self._omni_report()
            pump(0.3)
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

# Address bar (purpose URL): native keys, Surrounding Text sent only when asked for, a delete that
# Chrome drops under a suggestion is repaired. Purpose 0 again restores surrounding.
OMNI = {"Ru": "st-lang.org", "Tra": "nslate.google.com"}
url = App("omnibox", surrounding=True, client_commit=False, table=OMNI)
url.ic.set_content_type(5, 0)
pump(0.3)
url.type("Rus ")
results.append(check("url purpose: Rus gives Rú", url.text, "Rú "))
results.append(check("url purpose: no preedit", url.shown, []))
results.append(check("url purpose: no forward-key", url.fwd, []))
results.append(check("url purpose: RequireSurroundingText sent in URL fields", url.required > 0, True))
url.table = None
url.sugg = ""
url.ic.set_content_type(0, 0)
pump(0.3)
url._sync()
shown_before = len(url.shown)
url.type("Rus ")
results.append(check("purpose 0 again: no preedit", len(url.shown), shown_before))
results.append(check("purpose 0 again: surrounding delete used", url.deletes > 0, True))
results.append(check("purpose 0 again: word replaced in place", url.text, "Rú Rú "))
lat += url.latency
url.ic.destroy()

# A second address bar, created after the first one is gone (destroying a focused context
# would leave the other without focus in the service).
url2 = App("omnibox2", surrounding=True, client_commit=False, table=OMNI)
url2.ic.set_content_type(5, 0)
pump(0.3)
url2.type("Traanf ", lag=True)
results.append(check("url purpose: Traanf gives Trần", url2.text, "Trần "))
results.append(check("url purpose 2: no preedit", url2.shown, []))
results.append(check("url purpose 2: no forward-key", url2.fwd, []))
lat += url2.latency
url2.ic.destroy()

x11 = App("x11", surrounding=False, client_commit=False)
x11.type("nguowif ")
x11.type("dduwowcj")
x11.click()
results.append(check("x11 app: daemon commits preedit once on click", x11.text, "người được"))
lat += x11.latency
x11.ic.destroy()

lat.sort()
print(f"key round trip (client -> rustype-linux -> client): median {lat[len(lat)//2]*1e6:.0f} us, p95 {lat[int(len(lat)*0.95)]*1e6:.0f} us, n={len(lat)}")
print("ALL PASS" if all(results) else "SOME FAILED")
sys.exit(0 if all(results) else 1)

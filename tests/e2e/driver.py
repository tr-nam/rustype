#!/usr/bin/env python3
# End-to-end cases driven through the private stack (see run-host.sh). Stdlib + Gio/GLib only.
# Keys: org.gnome.Mutter.RemoteDesktop (NotifyKeyboardKeysym / NotifyPointerButton).
# Read-back: files the test apps write, and Chrome DevTools (WebSocket) for case e.
# usage: driver.py CASE WORD WORK_DIR LOGS_DIR SHELL_PID
import base64, glob, json, os, re, signal, socket, subprocess, sys, time, urllib.parse, urllib.request
import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

CASE, WORD, WORK, LOGS, SHELL_PID = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5])
HERE = os.path.dirname(os.path.abspath(__file__))
APPS = os.path.join(HERE, "apps")
OUT = os.path.join(WORK, "out")
os.makedirs(OUT, exist_ok=True)
RD = "org.gnome.Mutter.RemoteDesktop"
KEY_DELAY = 0.12
SHIFT, CTRL, RETURN, TAB, L, K, BACKSPACE, ALT = 0xffe1, 0xffe3, 0xff0d, 0xff09, 0x6c, 0x6b, 0xff08, 0xffe9
RESULTS = []
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)

def record(sub, expected, actual, ok, note=""):
    r = {"case": CASE, "sub": sub, "expected": expected, "actual": actual, "pass": bool(ok), "note": note}
    RESULTS.append(r)
    with open(os.path.join(LOGS, "results.jsonl"), "a", encoding="utf-8") as f:
        f.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"RESULT {CASE}/{sub} {'PASS' if ok else 'FAIL'} expected={expected!r} actual={actual!r} {note}", flush=True)

def record_known(sub, expected, actual, ok, note):
    """A documented limitation: it is expected to fail. If it passes, the run fails so the marker gets removed."""
    r = {"case": CASE, "sub": sub, "expected": expected, "actual": actual, "pass": not ok, "note": note}
    RESULTS.append(r)
    with open(os.path.join(LOGS, "results.jsonl"), "a", encoding="utf-8") as f:
        f.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"RESULT {CASE}/{sub} {'UNEXPECTED-PASS' if ok else 'KNOWN-FAIL'} expected={expected!r} actual={actual!r} {note}", flush=True)

def shell_alive():
    """A dead Shell is a failure, not a hang: the pid must exist and org.gnome.Shell must be owned."""
    try:
        os.kill(SHELL_PID, 0)
    except OSError:
        return False
    try:
        bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                      "NameHasOwner", GLib.Variant("(s)", ("org.gnome.Shell",)), GLib.VariantType("(b)"),
                      Gio.DBusCallFlags.NONE, -1, None)
        return True
    except Exception:
        return False

def shell_log_bad():
    try:
        return "should not call the delete-surrounding-text API" in open(os.path.join(LOGS, "shell.log"), errors="replace").read()
    except OSError:
        return False

def rd_session():
    r = bus.call_sync(RD, "/org/gnome/Mutter/RemoteDesktop", RD, "CreateSession", None,
                      GLib.VariantType("(o)"), Gio.DBusCallFlags.NONE, -1, None)
    path = r.unpack()[0]
    bus.call_sync(RD, path, RD + ".Session", "Start", None, None, Gio.DBusCallFlags.NONE, -1, None)
    return path

RDS = None
def key(ks, pressed):
    bus.call_sync(RD, RDS, RD + ".Session", "NotifyKeyboardKeysym", GLib.Variant("(ub)", (ks, pressed)),
                  None, Gio.DBusCallFlags.NONE, -1, None)

def tap(ks, delay=KEY_DELAY):
    """One key: press, sleep(delay), release, sleep(delay). A key takes 2*delay plus the D-Bus calls."""
    key(ks, True); time.sleep(delay); key(ks, False); time.sleep(delay)

def type_text(text, delay=KEY_DELAY):
    """delay is the sleep after each press and each release. The default (0.12 s) is about 0.24 s per key.
    Case g passes 0.002, which is about 4 ms per key before the D-Bus calls."""
    for ch in text:
        if ch == "\n": tap(RETURN, delay); continue
        if ch == "\t": tap(TAB, delay); continue
        if ch.isupper():
            key(SHIFT, True); time.sleep(delay); tap(ord(ch.lower()), delay); key(SHIFT, False); time.sleep(delay)
        else:
            tap(ord(ch), delay)

def click_center():
    """Mutter does not focus a fresh window by itself: a pointer click at the centre gives it focus."""
    for pressed in (True, False):
        bus.call_sync(RD, RDS, RD + ".Session", "NotifyPointerButton", GLib.Variant("(ib)", (272, pressed)),
                      None, Gio.DBusCallFlags.NONE, -1, None)
        time.sleep(0.2)
    time.sleep(0.8)

def read(name):
    try:
        return open(os.path.join(OUT, name), encoding="utf-8").read()
    except OSError:
        return None

def wait_file(name, timeout=30):
    end = time.time() + timeout
    while time.time() < end:
        if os.path.exists(os.path.join(OUT, name)): return True
        time.sleep(0.2)
    return False

def wait_focus(timeout=20):
    """Gives keyboard focus time to settle after the click. A bare Shift tap (no text) is sent so the first
    real key does not race the focus change; the focus.txt flag is recorded but not trusted alone."""
    end = time.time() + timeout
    while time.time() < end and read("focus.txt") != "active":
        time.sleep(0.2)
    tap(SHIFT); time.sleep(1.0)
    return read("focus.txt") == "active"

def start_app(argv, env_extra=None):
    """Starts an app in its own process group and records the group for the run's cleanup."""
    env = dict(os.environ)
    if env_extra: env.update(env_extra)
    p = subprocess.Popen(argv, env=env, start_new_session=True, stdin=subprocess.DEVNULL,
                         stdout=open(os.path.join(LOGS, "app-%s.log" % os.path.basename(argv[1] if len(argv) > 1 else argv[0])), "ab"),
                         stderr=subprocess.STDOUT)
    with open(os.path.join(LOGS, "pgids"), "a") as f: f.write(f"{p.pid}\n")
    return p

def app_alive(p):
    return p.poll() is None

def gtk_argv(script, *args):
    return ["python3", "-I", os.path.join(APPS, script)] + list(args)

# ---------- case a: real terminal, default config ----------
def case_a():
    p = start_app(gtk_argv("term.py", OUT))
    if not wait_file("ready", 30): record("a", "window", "no window", False); return
    time.sleep(1.0); click_center(); wait_focus()
    type_text("tieengs vieejt\n")
    time.sleep(2.0)
    got = (read("term.txt") or "").strip()
    record("a", "tiếng việt", got, got == "tiếng việt" and app_alive(p) and shell_alive() and not shell_log_bad(),
           f"shell_alive={shell_alive()} app_alive={app_alive(p)} delete_api_log={shell_log_bad()}")

# ---------- case b: GTK entry, app clears the field mid-word ----------
def case_b():
    p = start_app(gtk_argv("entry.py", OUT))
    if not wait_file("ready", 30): record("b", "window", "no window", False); return
    time.sleep(1.0); click_center(); wait_focus()
    type_text("vieej"); time.sleep(0.8)
    os.kill(p.pid, signal.SIGUSR1); time.sleep(0.8)   # out-of-band clear: no key event
    type_text("tr"); time.sleep(1.0)
    got = read("entry.txt")
    record("b1", "tr", got, got == "tr" and shell_alive() and not shell_log_bad(), f"shell_alive={shell_alive()}")
    type_text(" "); time.sleep(0.2)
    type_text("tieengs"); time.sleep(0.8)
    os.kill(p.pid, signal.SIGUSR1); time.sleep(0.8)   # out-of-band clear: no key event
    type_text("Traanf"); time.sleep(1.0)
    got = read("entry.txt")
    record("b2-alive", True, shell_alive() and app_alive(p), shell_alive() and not shell_log_bad() and app_alive(p),
           f"shell_alive={shell_alive()} app_alive={app_alive(p)}")
    # GTK reports an app-side change only with the next key's update, so the engine still holds the
    # old word when 'T' arrives. 'T' cannot extend it, the old word is restored, and the letters
    # land in the emptied field. b1 avoids this because its next key replaces nothing.
    record_known("b2", "Trần", got, got == "Trần", "GTK reports the clear only after the next key")

def tap_ctrl_k():
    key(CTRL, True); time.sleep(KEY_DELAY); tap(K); key(CTRL, False); time.sleep(KEY_DELAY)

# ---------- case c: focus moves from an entry with text to a terminal in the same window ----------
def case_c():
    p = start_app(gtk_argv("combo.py", OUT))
    if not wait_file("ready", 30): record("c", "window", "no window", False); return
    time.sleep(1.0); click_center(); wait_focus()
    type_text("tieengs"); time.sleep(0.8)
    tap(TAB); time.sleep(0.8)
    type_text("vieejt\n"); time.sleep(2.0)
    entry = read("entry.txt"); term = (read("term.txt") or "").strip()
    ok = entry == "tiếng" and term == "việt" and shell_alive() and not shell_log_bad() and app_alive(p)
    record("c", "entry 'tiếng', terminal 'việt'", {"entry": entry, "terminal": term}, ok,
           f"shell_alive={shell_alive()} app_alive={app_alive(p)}")

# ---------- case d: GTK basics ----------
def case_d():
    p = start_app(gtk_argv("entry.py", OUT))
    if not wait_file("ready", 30): record("d", "window", "no window", False); return
    time.sleep(1.0); click_center(); wait_focus()
    type_text("Tieengs Vieejt"); time.sleep(1.0)
    got = read("entry.txt")
    record("d1-sentence-caps", "Tiếng Việt", got, got == "Tiếng Việt", "")
    tap(BACKSPACE); time.sleep(0.8)
    got = read("entry.txt")
    record("d2-backspace", "Tiếng Việ", got, got == "Tiếng Việ", "")
    # Password field: plain text, no Telex conversion.
    tap(TAB); time.sleep(0.5)
    type_text("tieengs"); time.sleep(1.0)
    got = read("password.txt")
    record("d3-password-plain", "tieengs", got, got == "tieengs" and shell_alive() and app_alive(p),
           f"shell_alive={shell_alive()}")
    # d4: zero preedit after the d1 sentence (preedit-changed count on the entry's inner GtkText).
    pre = read("preedit.txt")
    record("d4-zero-preedit", "0", pre, pre == "0", "preedit-changed count after d1 sentence")
    # d5: focus leaves the entry and comes back; the next word must still be correct, with no preedit.
    # Focus: password -> entry (Shift+Tab), entry -> password (leaves the entry), password -> entry (back).
    shift_tab = lambda: (key(SHIFT, True), time.sleep(KEY_DELAY), tap(TAB), key(SHIFT, False), time.sleep(0.6))
    shift_tab(); tap(TAB); time.sleep(0.6); shift_tab()
    # GTK selects the whole entry when keyboard focus returns to it: End collapses the selection to the end.
    tap(0xff57); time.sleep(0.4)
    type_text(" vieejt"); time.sleep(1.0)
    got = read("entry.txt"); pre = read("preedit.txt")
    record("d5-focus-round-trip", "Tiếng Việ việt|preedit 0", f"{got}|preedit {pre}",
           got == "Tiếng Việ việt" and pre == "0" and shell_alive(), f"shell_alive={shell_alive()}")

# ---------- case e: Chrome, address bar with an inline suggestion, and an <input> ----------
CHROME_PORT = 9333
def chrome_flags(profile):
    return ["/usr/bin/google-chrome", "--ozone-platform=wayland", "--enable-wayland-ime",
            "--wayland-text-input-version=3", "--no-first-run", "--no-default-browser-check", "--disable-sync",
            "--disable-background-networking", "--disable-component-update", "--password-store=basic",
            "--disable-gpu", "--remote-debugging-address=127.0.0.1", f"--remote-debugging-port={CHROME_PORT}",
            "--host-resolver-rules=MAP rustype.test 127.0.0.1, MAP trade.test 127.0.0.1, MAP * ~NOTFOUND, "
            "EXCLUDE localhost, EXCLUDE 127.0.0.1",
            f"--user-data-dir={profile}", "--window-size=1000,700", "--window-position=0,0"]

class Cdp:
    def __init__(self, argv):
        self.proc = subprocess.Popen(argv, env=dict(os.environ, WAYLAND_DISPLAY="rustype-e2e-0"),
                                     start_new_session=True, stdin=subprocess.DEVNULL,
                                     stdout=open(os.path.join(LOGS, "chrome.log"), "ab"), stderr=subprocess.STDOUT)
        with open(os.path.join(LOGS, "pgids"), "a") as f: f.write(f"{self.proc.pid}\n")
        ver = None
        for _ in range(150):
            try:
                ver = json.load(urllib.request.urlopen(f"http://127.0.0.1:{CHROME_PORT}/json/version", timeout=1)); break
            except Exception: time.sleep(0.2)
        if ver is None: raise RuntimeError("no DevTools endpoint")
        u = urllib.parse.urlparse(ver["webSocketDebuggerUrl"])
        self.sock = socket.create_connection(("127.0.0.1", u.port), timeout=30)
        key_b = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall((f"GET {u.path} HTTP/1.1\r\nHost: 127.0.0.1:{u.port}\r\nUpgrade: websocket\r\n"
                           f"Connection: Upgrade\r\nSec-WebSocket-Key: {key_b}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        head = b""
        while b"\r\n\r\n" not in head: head += self.sock.recv(1)
        if b" 101 " not in head.split(b"\r\n")[0]: raise RuntimeError("websocket upgrade refused")
        self.id = 0
    def _exact(self, n):
        b = b""
        while len(b) < n:
            c = self.sock.recv(n - len(b))
            if not c: raise RuntimeError("devtools closed")
            b += c
        return b
    def _msg(self):
        while True:
            h = self._exact(2); op = h[0] & 0x0F; ln = h[1] & 0x7F
            if ln == 126: ln = int.from_bytes(self._exact(2), "big")
            elif ln == 127: ln = int.from_bytes(self._exact(8), "big")
            pl = self._exact(ln)
            if op == 1: return json.loads(pl)
            if op == 8: raise RuntimeError("devtools websocket closed")
    def send(self, method, params=None, session=None):
        self.id += 1
        msg = {"id": self.id, "method": method, "params": params or {}}
        if session: msg["sessionId"] = session
        data = json.dumps(msg).encode(); mask = os.urandom(4); n = len(data)
        head = bytes([0x81])
        if n < 126: head += bytes([0x80 | n])
        elif n < 65536: head += bytes([0x80 | 126]) + n.to_bytes(2, "big")
        else: head += bytes([0x80 | 127]) + n.to_bytes(8, "big")
        self.sock.sendall(head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))
        while True:
            m = self._msg()
            if m.get("id") == self.id:
                if "error" in m: raise RuntimeError(f"{method}: {m['error']}")
                return m.get("result", {})

def page_urls(cdp):
    return [t["url"] for t in cdp.send("Target.getTargets")["targetInfos"] if t["type"] == "page"]

def case_e():
    page = os.path.join(WORK, "page")
    os.makedirs(page, exist_ok=True)
    with open(os.path.join(page, "index.html"), "w", encoding="utf-8") as f:
        f.write('<!doctype html><meta charset="utf-8"><title>rustype e2e</title>'
                '<input id="i1"><br><input id="i2"><br><input id="i3">')
    srv = subprocess.Popen(["python3", "-I", "-m", "http.server", "8765", "--bind", "127.0.0.1", "-d", page],
                           start_new_session=True, stdin=subprocess.DEVNULL,
                           stdout=open(os.path.join(LOGS, "http.log"), "ab"), stderr=subprocess.STDOUT)
    with open(os.path.join(LOGS, "pgids"), "a") as f: f.write(f"{srv.pid}\n")
    time.sleep(1)
    profile = os.path.join(WORK, "chrome-profile")
    cdp = Cdp(chrome_flags(profile) + ["about:blank"])
    time.sleep(3)
    sess = None
    for t in cdp.send("Target.getTargets")["targetInfos"]:
        if t["type"] == "page":
            sess = cdp.send("Target.attachToTarget", {"targetId": t["targetId"], "flatten": True})["sessionId"]; break
    global RDS
    RDS = rd_session()
    time.sleep(1.0); click_center()
    # History: each host typed and entered twice, so Chrome can inline-complete it.
    for host in ["rustype.test", "trade.test"]:
        for _ in range(2):
            tap_ctrl_l()
            type_text(f"{host}:8765"); time.sleep(1.5); tap(RETURN); time.sleep(3.0)
    # The word in a fresh address bar, with the suggestion expected after its prefix.
    tap_ctrl_l(); time.sleep(0.5)
    type_text(WORD); time.sleep(2.0)
    tap(RETURN); time.sleep(4.0)
    urls = page_urls(cdp)
    queries = [urllib.parse.parse_qs(urllib.parse.urlparse(u).query).get("q", [None])[0] for u in urls]
    want = "Rú" if WORD == "Rus" else "Trần"
    record(f"e-address-{WORD}", want, queries, want in queries and shell_alive() and not shell_log_bad(),
           f"urls={urls[:1]} shell_alive={shell_alive()}")
    # An <input> field: the same word typed into a page field.
    cdp.send("Page.navigate", {"url": "http://127.0.0.1:8765/index.html"}, sess); time.sleep(2.5)
    cdp.send("Runtime.evaluate", {"expression": "document.getElementById('i1').focus()"}, sess); time.sleep(0.5)
    type_text(WORD + " "); time.sleep(1.0)
    got = cdp.send("Runtime.evaluate", {"expression": "document.getElementById('i1').value", "returnByValue": True}, sess)["result"].get("value")
    want_in = ("Rú " if WORD == "Rus" else "Trần ")
    record(f"e-input-{WORD}", want_in, got, got == want_in, "")
    try: cdp.proc.terminate()
    except Exception: pass

def tap_ctrl_l():
    key(CTRL, True); time.sleep(KEY_DELAY); tap(L); key(CTRL, False); time.sleep(KEY_DELAY)

# ---------- case f: the app replaces the text out of band, mid-word (SIGUSR2: 'tiếng việt', cursor at 0) ----------
# WORD selects the variant: "2" is the URL-purpose entry (f2), anything else is the plain entry (f1).
def case_f():
    url = WORD == "2"
    sub = "f2" if url else "f1"
    p = start_app(gtk_argv("entry.py", OUT, "url") if url else gtk_argv("entry.py", OUT))
    if not wait_file("ready", 30): record(sub, "window", "no window", False); return
    time.sleep(1.0); click_center(); wait_focus()
    if url:
        type_text("vieej"); time.sleep(0.8)
        os.kill(p.pid, signal.SIGUSR1); time.sleep(0.8)   # out-of-band clear: no key event
        type_text("tr"); time.sleep(1.0)
        got = read("entry.txt")
        alive = shell_alive() and app_alive(p)
        record("f2", "Shell and app alive", alive, alive and not shell_log_bad(),
               f"text={got!r} (URL field: only checked for crashes) shell_alive={shell_alive()} app_alive={app_alive(p)}")
    else:
        type_text("aa"); type_text(" "); tap(BACKSPACE); time.sleep(0.8)
        os.kill(p.pid, signal.SIGUSR2); time.sleep(0.8)  # out-of-band replace: no key event
        type_text("cm"); time.sleep(1.0)
        got = read("entry.txt")
        record("f1", "cmtiếng việt", got,
               got == "cmtiếng việt" and shell_alive() and app_alive(p) and not shell_log_bad(),
               f"shell_alive={shell_alive()} app_alive={app_alive(p)}")

# ---------- case g: fast typing right after a focus move (4 ms per key) ----------
def case_g():
    p = start_app(gtk_argv("two.py", OUT))
    if not wait_file("ready", 30): record("g", "window", "no window", False); return
    time.sleep(1.0); click_center(); wait_focus()
    prev = {"a": "", "b": ""}
    exact = rounds = keys = 0
    typing = 0.0
    for k in range(40):
        if not shell_alive(): break
        # Even rounds: focus goes a -> b (Tab). Odd rounds: b -> a (Shift+Tab). Keys follow the move at once.
        field = "b" if k % 2 == 0 else "a"
        if k % 2 == 0:
            tap(TAB, delay=0.002)
        else:
            key(SHIFT, True); time.sleep(0.002); tap(TAB, delay=0.002); key(SHIFT, False); time.sleep(0.002)
        s = time.time()
        type_text("tieengs vieejt ", delay=0.002)
        typing += time.time() - s
        keys += len("tieengs vieejt ")
        time.sleep(0.5)
        other = "a" if field == "b" else "b"
        got = read(f"field-{field}.txt") or ""
        rounds += 1
        if got == prev[field] + "tiếng việt " and (read(f"field-{other}.txt") or "") == prev[other]:
            exact += 1
        prev[field] = got
    alive = shell_alive() and app_alive(p)
    rate = keys / typing if typing > 0 else 0.0
    record("g-alive", True, alive, alive and not shell_log_bad(),
           f"exact {exact}/{rounds} rounds (not asserted); {keys} keys in {typing:.2f}s = {rate:.0f} keys/s; "
           f"shell_alive={shell_alive()} app_alive={app_alive(p)}")

# ---------- case h: a GTK4 Wayland window and a GTK3 X11 window (Xwayland), Alt+Tab between them ----------
def children_of(ppid, comm):
    out = []
    for d in os.listdir("/proc"):
        if not d.isdigit(): continue
        try:
            st = open(f"/proc/{d}/stat", encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        name = st[st.index("(") + 1:st.rindex(")")]
        if name == comm and int(st[st.rindex(")") + 2:].split()[1]) == ppid:
            out.append(int(d))
    return out

def xwayland_display(timeout=30):
    """The public display of the Xwayland that the Shell under test started, found through its own children
    and listening sockets. Mutter also listens on the next number for its own managed services (only the
    window manager may use it), so the lowest number is the public one. Never a display picked by number
    alone: /tmp/.X11-unix is shared with the real session."""
    end = time.time() + timeout
    while time.time() < end:
        for xw in children_of(SHELL_PID, "Xwayland"):
            inodes = set()
            try:
                for fd in os.listdir(f"/proc/{xw}/fd"):
                    try: target = os.readlink(f"/proc/{xw}/fd/{fd}")
                    except OSError: continue
                    if target.startswith("socket:["): inodes.add(target[8:-1])
            except OSError:
                continue
            nums = []
            for line in open("/proc/net/unix", encoding="utf-8", errors="replace"):
                parts = line.split()
                if len(parts) >= 8 and parts[6] in inodes:
                    m = re.search(r"/tmp/\.X11-unix/X(\d+)$", parts[7])
                    if m: nums.append(int(m.group(1)))
            if nums: return ":" + str(min(nums))
        time.sleep(0.3)
    return None

def alt_tab():
    key(ALT, True); time.sleep(KEY_DELAY); tap(TAB); key(ALT, False); time.sleep(0.8)

def focus_to(name, tries=3):
    """Alt+Tab (two windows: it always goes to the other one) until the named focus file says active."""
    for _ in range(tries):
        if read(name) == "active": return True
        alt_tab()
        end = time.time() + 3
        while time.time() < end and read(name) != "active": time.sleep(0.2)
    return read(name) == "active"

def case_h():
    wp = start_app(gtk_argv("entry.py", OUT))
    if not wait_file("ready", 30): record("h", "wayland window", "no window", False); return
    disp = xwayland_display()
    if not disp: record("h", "xwayland display", None, False, "no Xwayland under the Shell"); return
    env = {"DISPLAY": disp, "GDK_BACKEND": "x11", "GTK_IM_MODULE": "ibus",
           "IBUS_ADDRESS": f"unix:path={WORK}/xdg/ibus.sock"}
    xauth = glob.glob(os.path.join(WORK, "xdg", ".mutter-Xwaylandauth.*"))
    if xauth: env["XAUTHORITY"] = xauth[0]
    xp = start_app(gtk_argv("x11.py", OUT), env)
    if not wait_file("ready-x11", 30): record("h", "x11 window", "no window", False, f"display={disp}"); return
    time.sleep(1.0); click_center()
    if not focus_to("x11-focus.txt"): record("h", "x11 window focused", "not focused", False); return
    skip = None
    for word, want in [("vieejt", "việt"), ("nguowif", "người"), ("tieengs", "tiếng")]:
        if not shell_alive(): break
        if not focus_to("focus.txt"): record("h", "wayland window focused", "not focused", False); break
        type_text("ab "); time.sleep(0.8)
        if not focus_to("x11-focus.txt"): record("h", "x11 window focused", "not focused", False); break
        prev = read("x11-entry.txt") or ""
        type_text(word + " "); time.sleep(1.0)
        now = read("x11-entry.txt") or ""
        delta = now[len(prev):] if now.startswith(prev) else now
        if delta.isascii():
            # No conversion at all: the GTK3 IBus module is not in the path, so this says nothing about tones.
            skip = f"GTK3 IBus did not convert {word!r}: x11 field got raw {delta!r}"
            break
        # A plain check: this short sequence does not reproduce the wrong tones seen in a long stress run.
        record(f"h-x11-{word}", want + " ", delta, delta == want + " ",
               f"x11 display={disp} wayland entry={read('entry.txt')!r}")
    if skip: print(f"NOTE {CASE} SKIP {skip}", flush=True)
    alive = shell_alive() and app_alive(wp) and app_alive(xp)
    record("h-alive", True, alive, alive and not shell_log_bad(),
           f"shell_alive={shell_alive()} wayland_alive={app_alive(wp)} x11_alive={app_alive(xp)}"
           + (f" skip={skip!r}" if skip else ""))

CASES = {"a": case_a, "b": case_b, "c": case_c, "d": case_d, "e": case_e, "f": case_f, "g": case_g, "h": case_h}
if __name__ == "__main__":
    try:
        RDS = rd_session() if CASE != "e" else RDS
        if CASE != "e":
            time.sleep(0.5)
        CASES[CASE]()
    except Exception as e:
        record(CASE, "no exception", repr(e), False)
    if not shell_alive():
        record(CASE, "shell alive at end", "shell dead", False)
    sys.exit(0 if all(r["pass"] for r in RESULTS) and RESULTS else 1)

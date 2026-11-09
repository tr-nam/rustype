# GTK 4 test window: a plain entry and a password entry. Every change is written to OUT/entry.txt or
# OUT/password.txt. The plain entry is cleared OUT OF BAND (SIGUSR1, no key event), as the kit clears through
# D-Bus: the engine must survive a field change it did not cause. SIGUSR2 replaces its text out of band with
# 'tiếng việt' and puts the cursor at 0. The optional argument 'url' gives the plain entry input-purpose URL.
# preedit-changed on the entry's inner GtkText (get_delegate, as in the kit's probe) is counted into OUT/preedit.txt.
import os, signal, sys
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib

OUT = sys.argv[1]
URL = len(sys.argv) > 2 and sys.argv[2] == "url"

def write(name, text):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        f.write(text)

class App(Gtk.Application):
    def __init__(self):
        super().__init__(application_id="dev.rustype.e2e.entry")
        self.connect("activate", self.on_activate)
        self.preedits = 0

    def on_activate(self, app):
        win = Gtk.ApplicationWindow(application=app, title="rustype e2e entry")
        win.set_default_size(1000, 700)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin_top=12, margin_start=12)
        self.entry = Gtk.Entry()
        self.entry.connect("changed", lambda e: write("entry.txt", e.get_text()))
        if URL:
            self.entry.set_input_purpose(Gtk.InputPurpose.URL)
        self.password = Gtk.PasswordEntry()
        self.password.connect("changed", lambda e: write("password.txt", e.get_text()))
        # Preedit probe on the inner GtkText, as tests/rustype-linux-test-kit does (get_delegate on GTK4).
        self.entry.get_delegate().connect("preedit-changed", self.on_preedit)
        box.append(self.entry)
        box.append(self.password)
        win.set_child(box)
        win.connect("notify::is-active", lambda w, _p: write("focus.txt", "active" if w.is_active() else "inactive"))
        win.present()
        self.entry.grab_focus()
        # Out-of-band clear: no key event reaches the engine for this change.
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGUSR1, self.clear_entry)
        # Out-of-band replace: the text changes under the engine, the cursor goes to the start.
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGUSR2, self.replace_entry)
        write("entry.txt", ""); write("password.txt", ""); write("preedit.txt", "0")
        write("ready", "1")

    def on_preedit(self, _w, _s):
        self.preedits += 1
        write("preedit.txt", str(self.preedits))

    def clear_entry(self):
        self.entry.set_text("")
        return GLib.SOURCE_CONTINUE

    def replace_entry(self):
        self.entry.set_text("tiếng việt")
        self.entry.set_position(0)
        return GLib.SOURCE_CONTINUE

App().run(None)

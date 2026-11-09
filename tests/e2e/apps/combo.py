# GTK 3 + VTE window for case c: a Gtk.Entry above a terminal running `cat > OUT/term.txt`.
# Focus moves between them with Tab. Entry text goes to OUT/entry.txt on every change.
import os, sys
import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Vte", "2.91")
from gi.repository import Gtk, Vte, GLib

OUT = sys.argv[1]
def write(name, text):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        f.write(text)
win = Gtk.Window(title="rustype e2e combo")
win.set_default_size(1000, 700)
box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
entry = Gtk.Entry()
entry.connect("changed", lambda e: write("entry.txt", e.get_text()))
# GTK 3: the entry itself carries preedit-changed. Count into OUT/preedit.txt.
_pre = {"n": 0}
def _on_preedit(_w, _s):
    _pre["n"] += 1
    write("preedit.txt", str(_pre["n"]))
entry.connect("preedit-changed", _on_preedit)
term = Vte.Terminal()
term.spawn_sync(Vte.PtyFlags.DEFAULT, None, ["/bin/sh", "-c", f"cat > '{OUT}/term.txt'"],
                None, GLib.SpawnFlags.DEFAULT, None, None, None)
box.pack_start(entry, False, False, 0)
box.pack_start(term, True, True, 0)
win.add(box)
win.connect("destroy", Gtk.main_quit)
win.connect("notify::is-active", lambda w, _p: open(os.path.join(OUT, "focus.txt"), "w").write("active" if w.is_active() else "inactive"))
win.show_all()
entry.grab_focus()
write("ready", "1")
Gtk.main()

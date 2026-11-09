# GTK 3 test window for case h, meant to run on Xwayland (GDK_BACKEND=x11, GTK_IM_MODULE=ibus).
# Its entry goes to OUT/x11-entry.txt on every change; x11-focus.txt says whether the window is active.
import os, sys
import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk

OUT = sys.argv[1]

def write(name, text):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        f.write(text)

win = Gtk.Window(title="rustype e2e x11")
win.set_default_size(1000, 700)
box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
box.set_margin_top(12); box.set_margin_start(12)
entry = Gtk.Entry()
entry.connect("changed", lambda e: write("x11-entry.txt", e.get_text()))
box.pack_start(entry, False, False, 0)
win.add(box)
win.connect("destroy", Gtk.main_quit)
win.connect("notify::is-active", lambda w, _p: write("x11-focus.txt", "active" if w.is_active() else "inactive"))
win.show_all()
entry.grab_focus()
write("x11-entry.txt", "")
write("ready-x11", "1")
Gtk.main()

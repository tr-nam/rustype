# GTK 3 + VTE 2.91 window running `cat > OUT/term.txt`: what is typed in the terminal and sent with Enter
# arrives in the file. Used for the real-terminal case (a).
import os, sys
import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Vte", "2.91")
from gi.repository import Gtk, Vte, GLib

OUT = sys.argv[1]
win = Gtk.Window(title="rustype e2e terminal")
win.set_default_size(1000, 700)
term = Vte.Terminal()
win.add(term)
win.connect("destroy", Gtk.main_quit)
win.connect("notify::is-active", lambda w, _p: open(os.path.join(OUT, "focus.txt"), "w").write("active" if w.is_active() else "inactive"))
term.spawn_sync(Vte.PtyFlags.DEFAULT, None,
                ["/bin/sh", "-c", f"cat > '{OUT}/term.txt'"],
                None, GLib.SpawnFlags.DEFAULT, None, None, None)
win.show_all()
term.grab_focus()
with open(os.path.join(OUT, "ready"), "w") as f:
    f.write("1")
Gtk.main()

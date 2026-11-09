# GTK 4 test window for case g: two plain entries, Tab moves between them. Each entry's text goes to
# OUT/field-a.txt or OUT/field-b.txt on every change. focus.txt says whether the window is active.
import os, sys
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk

OUT = sys.argv[1]

def write(name, text):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        f.write(text)

class App(Gtk.Application):
    def __init__(self):
        super().__init__(application_id="dev.rustype.e2e.two")
        self.connect("activate", self.on_activate)

    def on_activate(self, app):
        win = Gtk.ApplicationWindow(application=app, title="rustype e2e two")
        win.set_default_size(1000, 700)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin_top=12, margin_start=12)
        a = Gtk.Entry()
        b = Gtk.Entry()
        a.connect("changed", lambda e: write("field-a.txt", e.get_text()))
        b.connect("changed", lambda e: write("field-b.txt", e.get_text()))
        box.append(a)
        box.append(b)
        win.set_child(box)
        win.connect("notify::is-active", lambda w, _p: write("focus.txt", "active" if w.is_active() else "inactive"))
        win.present()
        a.grab_focus()
        write("field-a.txt", ""); write("field-b.txt", "")
        write("ready", "1")

App().run(None)

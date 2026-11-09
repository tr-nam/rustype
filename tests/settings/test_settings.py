#!/usr/bin/env python3
"""Data-layer tests for tools/rustype-settings: config, macro and words files.

Run from anywhere:  python3 tests/settings/test_settings.py
Exit status is nonzero when any test fails. unittest.expectedFailure is not used for any test here;
a product bug found later gets a marker that names the bug, and the marker is removed with the fix.

Safety: every path the app uses is redirected into a private temp directory before the settings
module is imported, and the run aborts unless the resolved config path lies inside that directory.
GTK is never loaded. A fake `gi` is installed, so the window class can be built without a display.

Optional environment variables:
  RUSTYPE_LINUX_BIN      built rustype-linux. Enables the engine cross-checks. If unset, those
                         checks are skipped with a visible message.
  RUSTYPE_SETTINGS_PATH  settings script to load instead of tools/rustype-settings. Used to run the
                         suite against a deliberately broken copy.
"""

import atexit
import importlib.machinery
import importlib.util
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
ENGINE_CONFIG_RS = ROOT / "rustype-linux" / "src" / "config.rs"

# 1. Private sandbox. These variables must be set before the settings module is imported, because
#    it reads them at import time.
SANDBOX = pathlib.Path(tempfile.mkdtemp(prefix="rustype-settings-test-")).resolve()
atexit.register(shutil.rmtree, SANDBOX, True)
FAKE_HOME = SANDBOX / "home"
FAKE_XDG_RUNTIME = SANDBOX / "run"
for _dir in (FAKE_HOME, FAKE_HOME / ".config", FAKE_HOME / ".cache", FAKE_XDG_RUNTIME):
    _dir.mkdir(parents=True, exist_ok=True)
FAKE_XDG_RUNTIME.chmod(0o700)
os.environ["HOME"] = str(FAKE_HOME)
os.environ["XDG_CONFIG_HOME"] = str(FAKE_HOME / ".config")
os.environ["XDG_CACHE_HOME"] = str(FAKE_HOME / ".cache")
os.environ["XDG_DATA_HOME"] = str(FAKE_HOME / ".local" / "share")
os.environ["XDG_RUNTIME_DIR"] = str(FAKE_XDG_RUNTIME)


def inside(path, root=SANDBOX):
    try:
        pathlib.Path(path).resolve().relative_to(root)
        return True
    except ValueError:
        return False


# 2. Fake gi, so that tools/rustype-settings can be imported without GTK. Every name the module
#    touches at import time exists. Classes the tests inspect record what the window did to them.
class _Anything:
    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        return _Anything()

    def __call__(self, *args, **kwargs):
        return _Anything()


class _AnyMeta(type):
    def __getattr__(cls, name):
        if name.startswith("__"):
            raise AttributeError(name)
        sub = _AnyMeta(name, (_Fake,), {})
        setattr(cls, name, sub)
        return sub


class _Fake(metaclass=_AnyMeta):
    def __init__(self, *args, **kwargs):
        self.kwargs = kwargs
        self.signals = {}

    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        return _Anything()

    def connect(self, signal, callback, *_):
        self.signals.setdefault(signal, []).append(callback)
        return 1

    def emit(self, signal, *args):
        for callback in list(self.signals.get(signal, [])):
            callback(self, *args)


class FakeEntry(_Fake):
    instances = []

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._text = ""
        FakeEntry.instances.append(self)

    def get_text(self):
        return self._text

    def set_text(self, text):
        self._text = text


class FakeDialog(_Fake):
    instances = []

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        FakeDialog.instances.append(self)


class FakeButton(_Fake):
    instances = []

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        FakeButton.instances.append(self)


class FakeGroup(_Fake):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.children = []

    def add(self, child):
        self.children.append(child)

    def remove(self, child):
        self.children.remove(child)

    def titles(self):
        return [child.title for child in self.children]


class FakeRow(_Fake):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.title = kwargs.get("title")


class FakeSwitchRow(_Fake):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.active = bool(kwargs.get("active", False))

    def get_active(self):
        return self.active

    def set_active(self, value):
        self.active = value
        self.emit("notify::active", None)


class FakeComboRow(_Fake):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.selected = 0

    def get_selected(self):
        return self.selected

    def set_selected(self, index):
        self.selected = index
        self.emit("notify::selected", None)


class FakeToast(_Fake):
    @classmethod

    def new(cls, message):
        toast = cls()
        toast.message = message
        return toast


class FakeWindow(_Fake):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.toasts = []

    def add_toast(self, toast):
        self.toasts.append(toast.message)


class _Namespace(metaclass=_AnyMeta):
    pass


def _install_fake_gi():
    Adw = type("Adw", (_Namespace,), {})
    Gtk = type("Gtk", (_Namespace,), {})
    Adw.PreferencesWindow = FakeWindow
    Adw.MessageDialog = FakeDialog
    Adw.ComboRow = FakeComboRow
    Adw.SwitchRow = FakeSwitchRow
    Adw.ActionRow = FakeRow
    Adw.PreferencesGroup = FakeGroup
    Adw.Toast = FakeToast
    Gtk.Entry = FakeEntry
    Gtk.Button = FakeButton
    repository = types.ModuleType("gi.repository")
    for name, value in (
        ("Adw", Adw),
        ("Gtk", Gtk),
        ("Gdk", _Namespace),
        ("Gio", _Namespace),
        ("GLib", _Namespace),
    ):
        setattr(repository, name, value)
    gi = types.ModuleType("gi")
    gi.require_version = lambda *args, **kwargs: None
    gi.repository = repository
    sys.modules["gi"] = gi
    sys.modules["gi.repository"] = repository


_install_fake_gi()

# 3. Load the settings script by path. Its module-level constants now point into SANDBOX.
SETTINGS_PATH = pathlib.Path(
    os.environ.get("RUSTYPE_SETTINGS_PATH") or ROOT / "tools" / "rustype-settings"
).resolve()
_loader = importlib.machinery.SourceFileLoader("rustype_settings_under_test", str(SETTINGS_PATH))
_spec = importlib.util.spec_from_loader(_loader.name, _loader)
mod = importlib.util.module_from_spec(_spec)
_loader.exec_module(mod)

# 4. Refuse to run unless every file the app would touch is inside the sandbox.
for _path in (mod.CONFIG_DIR, mod.CONFIG_FILE, mod.MACRO_FILE, mod.WORDS_FILE):
    if not inside(_path):
        raise SystemExit(
            f"refusing to run: {_path} resolves outside the test sandbox {SANDBOX}"
        )

# 5. Engine binary, when given. A path that is set but missing is an error, not a skip.
RUST_BIN = os.environ.get("RUSTYPE_LINUX_BIN") or None
if RUST_BIN is not None and not os.access(RUST_BIN, os.X_OK):
    raise SystemExit(f"RUSTYPE_LINUX_BIN={RUST_BIN} is not an executable file")
ENGINE_SKIP = "RUSTYPE_LINUX_BIN is not set: engine cross-check skipped"
if RUST_BIN is None:
    print(f"SKIP group engine-binary: {ENGINE_SKIP}", file=sys.stderr)

OPTION_ATTRS = (
    "mode",
    "input_method",
    "modern_orthography",
    "auto_restore_english",
    "remember_state_per_app",
    "enable_macro",
    "free_tone_order",
    "toggle_shortcut",
    "simple_telex",
    "quick_telex",
    "auto_capitalize",
    "custom_words",
    "app_modes",
    "macros",
)
DEFAULTS = {
    "mode": "auto",
    "input_method": "telex",
    "modern_orthography": False,
    "auto_restore_english": True,
    "remember_state_per_app": True,
    "enable_macro": False,
    "free_tone_order": True,
    "toggle_shortcut": "ctrl_shift",
    "simple_telex": False,
    "quick_telex": False,
    "auto_capitalize": False,
    "custom_words": [],
    "app_modes": {},
    "macros": {},
}


def snapshot(cm):
    return {name: getattr(cm, name) for name in OPTION_ATTRS}


def set_all_options(cm):
    """Sets every managed option to a value that differs from its default."""
    cm.mode = "preedit"
    cm.input_method = "vni"
    cm.modern_orthography = True
    cm.auto_restore_english = False
    cm.remember_state_per_app = False
    cm.enable_macro = True
    cm.free_tone_order = False
    cm.toggle_shortcut = "off"
    cm.simple_telex = True
    cm.quick_telex = True
    cm.auto_capitalize = True
    cm.custom_words = ["this", "test", "github"]
    cm.app_modes = {"org.gnome.terminal": "off", "kitty": "preedit", "google-chrome": "direct"}
    cm.macros = {"vn": "Việt Nam", "hn": "Hà Nội"}


def reset_config_dir():
    if not inside(mod.CONFIG_DIR):
        raise SystemExit("refusing to reset a directory outside the sandbox")
    shutil.rmtree(mod.CONFIG_DIR, ignore_errors=True)
    mod.CONFIG_DIR.mkdir(parents=True)


def put(path, data):
    if not inside(path):
        raise SystemExit(f"refusing to write {path}: outside the sandbox")
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, bytes):
        path.write_bytes(data)
    else:
        path.write_text(data, encoding="utf-8")


def load_cm():
    return mod.ConfigManager()


def open_window():
    """Builds the real preferences window on top of the fake GTK classes."""
    cm = mod.ConfigManager()
    with mock.patch.object(mod, "find_engine", return_value=None):
        return mod.RustypePreferencesWindow(None, cm)


def submit_dialog(open_method, *texts):
    """Calls a bound add handler, fills its entries in creation order, then answers "add"."""
    FakeEntry.instances.clear()
    FakeDialog.instances.clear()
    open_method(None)
    entries = list(FakeEntry.instances)
    if len(entries) != len(texts):
        raise AssertionError(f"expected {len(texts)} entries, got {len(entries)}")
    for entry, text in zip(entries, texts):
        entry.set_text(text)
    FakeDialog.instances[-1].emit("response", "add")


def delete_button(value):
    """The trash button the window built for value. Each one binds its value as a default arg."""
    found = [
        b
        for b in FakeButton.instances
        if b.kwargs.get("icon_name") == "user-trash-symbolic"
        and b.signals.get("clicked")
        and b.signals["clicked"][0].__defaults__ == (value,)
    ]
    if not found:
        raise AssertionError(f"no delete button for {value!r}")
    return found[-1]


class SettingsTestCase(unittest.TestCase):
    def setUp(self):
        reset_config_dir()
        FakeButton.instances.clear()


class SandboxTests(SettingsTestCase):
    def test_all_paths_resolve_inside_the_sandbox(self):
        for path in (mod.CONFIG_DIR, mod.CONFIG_FILE, mod.MACRO_FILE, mod.WORDS_FILE):
            self.assertTrue(inside(path), path)
        self.assertTrue(inside(pathlib.Path.home()))


class ConfigRoundTripTests(SettingsTestCase):
    def test_defaults_without_any_file(self):
        self.assertEqual(snapshot(load_cm()), DEFAULTS)

    def test_every_managed_option_round_trips(self):
        cm = load_cm()
        set_all_options(cm)
        self.assertNotEqual(snapshot(cm), DEFAULTS, "set_all_options must change every option")
        cm.save()
        self.assertEqual(snapshot(load_cm()), snapshot(cm))

    def test_every_managed_key_is_written_exactly_once(self):
        cm = load_cm()
        set_all_options(cm)
        cm.save()
        text = mod.CONFIG_FILE.read_text(encoding="utf-8")
        for key in mod.TOP_KEYS:
            count = sum(
                1
                for line in text.splitlines()
                if mod.config_key_value(line) and mod.config_key_value(line)[0] == key
            )
            self.assertEqual(count, 1, key)

    def test_each_mode_and_toggle_value_round_trips(self):
        for mode in ("auto", "preedit", "direct"):
            for toggle in ("ctrl_shift", "off"):
                with self.subTest(mode=mode, toggle=toggle):
                    reset_config_dir()
                    cm = load_cm()
                    cm.mode, cm.toggle_shortcut = mode, toggle
                    cm.save()
                    reloaded = load_cm()
                    self.assertEqual((reloaded.mode, reloaded.toggle_shortcut), (mode, toggle))

    def test_off_app_mode_round_trips_with_its_value(self):
        put(mod.CONFIG_FILE, '[app_modes]\n"org.gnome.Terminal" = off\n')
        cm = load_cm()
        self.assertEqual(cm.app_modes, {"org.gnome.Terminal": "off"})
        cm.save()
        self.assertEqual(load_cm().app_modes, {"org.gnome.Terminal": "off"})

    def test_legacy_boolean_spellings_are_read(self):
        put(
            mod.CONFIG_FILE,
            "modern_orthography = yes\n"
            "auto_restore_english = on\n"
            "remember_state_per_app = 1\n"
            "enable_macro = TRUE\n"
            "free_tone_order = No\n"
            "simple_telex = 0\n"
            "quick_telex = off\n"
            "auto_capitalize = Yes\n",
        )
        cm = load_cm()
        self.assertEqual(
            (
                cm.modern_orthography,
                cm.auto_restore_english,
                cm.remember_state_per_app,
                cm.enable_macro,
                cm.free_tone_order,
                cm.simple_telex,
                cm.quick_telex,
                cm.auto_capitalize,
            ),
            (True, True, True, True, False, False, False, True),
        )


class PreservationTests(SettingsTestCase):
    ORIGINAL = (
        "# my notes\n"
        "mode = auto\n"
        "future_option = 42  # keep me\n"
        "free_tone_order = off\n"
        "\n"
        "[experimental]\n"
        "x = 1 # note\n"
        "# a comment in the section\n"
        "\n"
        "[app_modes]\n"
        "# per-app\n"
        '"org.gnome.Ptyxis" = preedit\n'
        '"org.gnome.Terminal" = direct\n'
        "\n"
        "[macros]\n"
        '"ck" = "c # k"\n'
    )

    def test_unknown_keys_sections_and_comments_survive_save(self):
        put(mod.CONFIG_FILE, self.ORIGINAL)
        cm = load_cm()
        cm.enable_macro = True
        cm.save()
        text = mod.CONFIG_FILE.read_text(encoding="utf-8")
        self.assertTrue(text.startswith("# my notes\n"))
        self.assertIn("future_option = 42  # keep me", text)
        self.assertIn("[experimental]\nx = 1 # note\n# a comment in the section\n", text)
        self.assertIn("# per-app", text)
        self.assertIn('"ck" = "c # k"', text)
        self.assertIn("enable_macro = true", text)
        self.assertIn("free_tone_order = false", text)

    def test_every_untouched_line_is_still_present_after_save(self):
        put(mod.CONFIG_FILE, self.ORIGINAL)
        cm = load_cm()
        cm.simple_telex = True
        cm.save()
        after = mod.CONFIG_FILE.read_text(encoding="utf-8").splitlines()
        managed_prefixes = tuple(f"{k} " for k in mod.TOP_KEYS) + ("free_tone_order",)
        for line in self.ORIGINAL.splitlines():
            if line.startswith(managed_prefixes):
                continue
            self.assertIn(line, after)

    def test_unchanged_managed_line_keeps_its_exact_text(self):
        put(mod.CONFIG_FILE, "mode = auto   # chú thích\n")
        load_cm().save()
        self.assertIn("mode = auto   # chú thích", mod.CONFIG_FILE.read_text(encoding="utf-8"))

    def test_save_is_idempotent(self):
        put(mod.CONFIG_FILE, self.ORIGINAL)
        load_cm().save()
        first = mod.CONFIG_FILE.read_text(encoding="utf-8")
        load_cm().save()
        self.assertEqual(mod.CONFIG_FILE.read_text(encoding="utf-8"), first)

    def test_duplicate_managed_key_resolves_to_one_line_with_last_value(self):
        # The engine keeps the last occurrence. The saved file must keep only one line with that value.
        put(mod.CONFIG_FILE, "mode = preedit\nmode = direct\n")
        cm = load_cm()
        self.assertEqual(cm.mode, "direct")
        cm.save()
        lines = [l for l in mod.CONFIG_FILE.read_text(encoding="utf-8").splitlines()
                 if mod.config_key_value(l) and mod.config_key_value(l)[0] == "mode"]
        self.assertEqual(lines, ["mode = direct"])

    def test_missing_managed_keys_are_added_before_the_first_section(self):
        put(mod.CONFIG_FILE, "# only a comment\n[macros]\n\"vn\" = \"Việt Nam\"\n")
        cm = load_cm()
        cm.save()
        head = mod.CONFIG_FILE.read_text(encoding="utf-8").split("[macros]")[0]
        for key in mod.TOP_KEYS:
            self.assertIn(f"{key} = ", head, key)

    def test_dotted_app_mode_key_moves_into_app_modes(self):
        # The value is stored under [app_modes] and the dotted line is dropped (config.rs:333-336).
        put(mod.CONFIG_FILE, "app_mode.kitty = preedit\n")
        load_cm().save()
        text = mod.CONFIG_FILE.read_text(encoding="utf-8")
        self.assertNotIn("app_mode.", text)
        self.assertEqual(load_cm().app_modes, {"kitty": "preedit"})

    def test_symlinked_config_keeps_the_link_and_updates_the_target(self):
        target = mod.CONFIG_DIR / "real_config"
        put(target, "mode = auto\n")
        mod.CONFIG_FILE.symlink_to(target)
        cm = load_cm()
        cm.enable_macro = True
        cm.save()
        self.assertTrue(mod.CONFIG_FILE.is_symlink())
        self.assertIn("enable_macro = true", target.read_text(encoding="utf-8"))
        self.assertFalse((mod.CONFIG_DIR / ".real_config.tmp").exists())

    def test_no_temp_file_is_left_after_save(self):
        load_cm().save()
        self.assertEqual(sorted(p.name for p in mod.CONFIG_DIR.iterdir()), ["config"])


class RobustnessTests(SettingsTestCase):
    def test_missing_file_gives_defaults(self):
        self.assertEqual(snapshot(load_cm()), DEFAULTS)

    def test_empty_and_blank_files_give_defaults(self):
        for text in ("", "\n\n   \n\t\n"):
            with self.subTest(text=text):
                put(mod.CONFIG_FILE, text)
                self.assertEqual(snapshot(load_cm()), DEFAULTS)

    def test_malformed_lines_are_skipped_and_valid_lines_still_load(self):
        put(
            mod.CONFIG_FILE,
            "garbage line with no equals\n"
            "=novalue\n"
            "[unclosed\n"
            "mode preedit\n"
            "]\n"
            "simple_telex = true\n",
        )
        cm = load_cm()
        self.assertEqual(cm.mode, "auto")
        self.assertTrue(cm.simple_telex)

    def test_crlf_line_endings_load(self):
        put(mod.CONFIG_FILE, b"mode = preedit\r\nsimple_telex = true\r\n[macros]\r\nvn = \"Vi\"\r\n")
        cm = load_cm()
        self.assertEqual((cm.mode, cm.simple_telex, cm.macros), ("preedit", True, {"vn": "Vi"}))

    def test_very_long_lines_round_trip(self):
        long_word = "w" * 100_000
        long_value = "v" * 100_000
        cm = load_cm()
        cm.custom_words = [long_word, "test"]
        cm.macros = {"long": long_value}
        cm.save()
        reloaded = load_cm()
        self.assertEqual(reloaded.custom_words, [long_word, "test"])
        self.assertEqual(reloaded.macros, {"long": long_value})

    def test_macro_file_malformed_lines_are_skipped(self):
        put(mod.MACRO_FILE, "no colon here\n: no key\nok: yes\n# comment: nope\n")
        self.assertEqual(load_cm().macros, {"ok": "yes"})

    def test_unknown_toggle_value_leaves_the_switch_on(self):
        # Unknown toggle values are not "off", so the engine (CtrlShift) and the switch agree.
        put(mod.CONFIG_FILE, "toggle_shortcut = bogus\n")
        self.assertTrue(open_window().sw_toggle.get_active())

    # ---- Known bugs: unknown or invalid values disagree with the engine or crash ----

    def test_undecodable_config_gives_defaults(self):
        # config.rs falls back to defaults when the file is not UTF-8 text.
        put(mod.CONFIG_FILE, b"\xff\xfe mode = preedit\n")
        self.assertEqual(snapshot(load_cm()), DEFAULTS)

    def test_undecodable_config_is_moved_aside_on_save(self):
        original = b"\xff\xfe mode = preedit\n"
        put(mod.CONFIG_FILE, original)
        cm = load_cm()
        cm.simple_telex = True
        cm.save()
        self.assertEqual((mod.CONFIG_DIR / "config.invalid").read_bytes(), original)
        self.assertTrue(load_cm().simple_telex)

    def test_undecodable_macro_and_words_files_are_skipped(self):
        # config.rs ignores a file it cannot read; the app must not crash on it either.
        put(mod.MACRO_FILE, b"vn: \xff\xfe\n")
        put(mod.WORDS_FILE, b"github\n\xff\n")
        cm = load_cm()
        self.assertEqual(cm.macros, {})
        self.assertEqual(cm.custom_words, [])

    def test_global_mode_off_is_treated_as_auto(self):
        # The engine maps a global `mode = off` to auto (config.rs:266-270).
        put(mod.CONFIG_FILE, "mode = off\n")
        self.assertEqual(load_cm().mode, "auto")

    def test_mode_value_is_case_insensitive(self):
        # The engine lowercases the mode value (config.rs:232).
        put(mod.CONFIG_FILE, "mode = Preedit\n")
        self.assertEqual(load_cm().mode, "preedit")

    def test_unknown_input_method_falls_back_to_telex(self):
        # The engine uses Telex for any value other than vni (config.rs:273-277).
        put(mod.CONFIG_FILE, "input_method = bogus\n")
        self.assertEqual(load_cm().input_method, "telex")

    def test_unknown_remember_state_value_keeps_default(self):
        # An unrecognised value keeps the default, as parse_bool does (config.rs:294-298).
        put(mod.CONFIG_FILE, "remember_state_per_app = maybe\n")
        self.assertTrue(load_cm().remember_state_per_app)

    def test_unknown_free_tone_value_keeps_default(self):
        # An unrecognised value keeps the default (config.rs:289-293).
        put(mod.CONFIG_FILE, "free_tone_order = bogus\n")
        self.assertTrue(load_cm().free_tone_order)

    def test_unknown_auto_restore_value_keeps_default(self):
        # An unrecognised value keeps the default (config.rs:284-288).
        put(mod.CONFIG_FILE, "auto_restore_english = maybe\n")
        self.assertTrue(load_cm().auto_restore_english)

    def test_empty_macro_value_is_not_a_macro(self):
        # The engine drops a macro line with an empty value (config.rs:356).
        put(mod.MACRO_FILE, "vn: Việt Nam\nhn:\n")
        self.assertNotIn("hn", load_cm().macros)


class MacroAndWordTests(SettingsTestCase):
    def test_macro_from_macro_file_is_deleted_and_stays_deleted(self):
        put(mod.MACRO_FILE, "vn: Việt Nam\nhn: Hà Nội\n")
        open_window()
        delete_button("vn").emit("clicked")
        reloaded = load_cm()
        self.assertNotIn("vn", reloaded.macros)
        self.assertEqual(reloaded.macros.get("hn"), "Hà Nội")
        self.assertNotIn("vn:", mod.MACRO_FILE.read_text(encoding="utf-8"))

    def test_macro_from_config_section_is_deleted_and_stays_deleted(self):
        put(mod.CONFIG_FILE, '[macros]\n"vn" = "Việt Nam"\n"hn" = "Hà Nội"\n')
        open_window()
        delete_button("vn").emit("clicked")
        self.assertNotIn("vn", load_cm().macros)
        self.assertEqual(load_cm().macros.get("hn"), "Hà Nội")

    def test_macro_defined_in_both_files_is_gone_after_delete(self):
        put(mod.CONFIG_FILE, '[macros]\n"vn" = "Việt Nam"\n')
        put(mod.MACRO_FILE, "vn: Việt Nam\n")
        open_window()
        delete_button("vn").emit("clicked")
        self.assertNotIn("vn", load_cm().macros)
        self.assertNotIn("vn", mod.CONFIG_FILE.read_text(encoding="utf-8"))
        self.assertNotIn("vn:", mod.MACRO_FILE.read_text(encoding="utf-8"))

    def test_added_macro_persists_with_unicode(self):
        win = open_window()
        submit_dialog(win.on_add_macro_clicked, "đc", "được")
        self.assertEqual(load_cm().macros["đc"], "được")

    def test_macro_value_with_separators_round_trips(self):
        win = open_window()
        submit_dialog(win.on_add_macro_clicked, "x", "a: b, c # d")
        self.assertEqual(load_cm().macros["x"], "a: b, c # d")

    def test_readding_a_macro_key_that_lives_in_the_macro_file_takes_effect(self):
        # Adding a macro also removes its line from the macro file, which would win on load (config.rs:349-371).
        put(mod.MACRO_FILE, "vn: Việt Nam\n")
        win = open_window()
        submit_dialog(win.on_add_macro_clicked, "vn", "Việt Nam Mới")
        self.assertEqual(load_cm().macros["vn"], "Việt Nam Mới")

    def test_macro_value_with_double_quote_survives_reload(self):
        # unquote strips one pair of quotes only, so embedded quotes survive (config.rs:255).
        win = open_window()
        submit_dialog(win.on_add_macro_clicked, "nh", 'Nói "hi"')
        self.assertEqual(load_cm().macros["nh"], 'Nói "hi"')

    def test_deleted_macro_row_leaves_the_list(self):
        # The list is rebuilt after a delete.
        put(mod.CONFIG_FILE, '[macros]\n"vn" = "Việt Nam"\n')
        win = open_window()
        delete_button("vn").emit("clicked")
        self.assertNotIn("vn", win.grp_macro_list.titles())

    def test_words_are_lowercased_and_duplicates_skipped(self):
        win = open_window()
        for text in ("GitHub", "github", "  Test  "):
            submit_dialog(win.on_add_word_clicked, text)
        self.assertEqual(load_cm().custom_words, ["github", "test"])

    def test_deleting_a_word_removes_every_copy_from_the_words_file(self):
        put(mod.WORDS_FILE, "github\n# keep this comment\ngithub  # duplicate\ntest\n")
        cm = load_cm()
        self.assertEqual(cm.custom_words, ["github", "test"])
        open_window()
        delete_button("github").emit("clicked")
        self.assertNotIn("github", load_cm().custom_words)
        text = mod.WORDS_FILE.read_text(encoding="utf-8")
        self.assertNotIn("github", text)
        self.assertIn("# keep this comment", text)
        self.assertIn("test", text)

    def test_deleting_a_word_from_the_config_list_persists(self):
        put(mod.CONFIG_FILE, 'custom_words = "this, test, github"\n')
        open_window()
        delete_button("test").emit("clicked")
        self.assertEqual(load_cm().custom_words, ["this", "github"])

    def test_unicode_word_round_trips(self):
        win = open_window()
        submit_dialog(win.on_add_word_clicked, "résumé")
        self.assertIn("résumé", load_cm().custom_words)

    def test_word_with_comma_is_refused(self):
        # The custom_words line is comma separated, so a word with a comma cannot be stored.
        win = open_window()
        submit_dialog(win.on_add_word_clicked, "a,b")
        self.assertEqual(win.cfg.custom_words, [])
        self.assertEqual(load_cm().custom_words, [])
        self.assertIn("dấu phẩy", win.toasts[-1])

    def test_deleted_word_row_leaves_the_list(self):
        # The list is rebuilt after a delete.
        put(mod.WORDS_FILE, "github\ntest\n")
        win = open_window()
        delete_button("github").emit("clicked")
        self.assertNotIn("github", win.grp_words.titles())

    # Import and export of Unikey-style tables: the settings app has no such feature (grep finds
    # none in tools/rustype-settings or rustype-linux/src), so there is nothing to test.


class AtomicWriteTests(SettingsTestCase):
    def test_failed_replace_keeps_the_original_file(self):
        put(mod.CONFIG_FILE, "mode = preedit\n# keep\n")
        cm = load_cm()
        cm.simple_telex = True
        with mock.patch.object(mod.os, "replace", side_effect=OSError("boom")):
            with self.assertRaises(OSError):
                cm.save()
        self.assertEqual(mod.CONFIG_FILE.read_text(encoding="utf-8"), "mode = preedit\n# keep\n")
        self.assertFalse((mod.CONFIG_DIR / ".config.tmp").exists())

    def test_failed_rewrite_reports_the_error_and_keeps_the_file(self):
        put(mod.MACRO_FILE, "vn: Việt Nam\n")
        with mock.patch.object(mod.os, "replace", side_effect=OSError("disk full")):
            error = mod.rewrite_file(mod.MACRO_FILE, lambda t: mod.drop_macro_lines(t, "vn"))
        self.assertIn("disk full", error)
        self.assertEqual(mod.MACRO_FILE.read_text(encoding="utf-8"), "vn: Việt Nam\n")
        self.assertFalse((mod.CONFIG_DIR / ".macro.tmp").exists())


class EngineAgreementSourceTests(unittest.TestCase):
    """Compares the names and spellings in rustype-linux/src/config.rs with the Python code. This
    reads the source only, so it runs even without the built binary."""

    @classmethod

    def setUpClass(cls):
        if not ENGINE_CONFIG_RS.exists():
            raise unittest.SkipTest(f"{ENGINE_CONFIG_RS} not found")
        cls.src = ENGINE_CONFIG_RS.read_text(encoding="utf-8")

    def test_managed_key_names_match_the_engine_match_arms(self):
        # The top-level arms of `match k` sit at 12 spaces. Nested `match v` arms sit deeper.
        block = self.src[self.src.index("match k {"): self.src.index('other if other.starts_with("app_mode.")')]
        engine_keys = set(re.findall(r'^ {12}"([a-z_]+)"\s*=>', block, re.M))
        self.assertEqual(set(mod.TOP_KEYS), engine_keys)

    def test_section_names_match_the_engine(self):
        self.assertEqual(
            set(re.findall(r'section == "([a-z_]+)"', self.src)), {"app_modes", "macros"}
        )

    def test_true_spellings_match_the_engine(self):
        line = next(l for l in self.src.splitlines() if "=> Some(true)" in l)
        engine_true = set(re.findall(r'"([a-z0-9]+)"', line))
        self.assertEqual(engine_true, {"true", "yes", "on", "1"})
        false_line = next(l for l in self.src.splitlines() if "=> Some(false)" in l)
        self.assertEqual(set(re.findall(r'"([a-z0-9]+)"', false_line)), {"false", "no", "off", "0"})
        for spelling in engine_true:
            put(mod.CONFIG_FILE, f"simple_telex = {spelling}\n")
            self.assertTrue(load_cm().simple_telex, spelling)


@unittest.skipIf(RUST_BIN is None, ENGINE_SKIP)
class EngineBinaryTests(SettingsTestCase):
    def run_engine(self, args, fresh=True):
        root = SANDBOX / "engine" / self._testMethodName
        if fresh:
            shutil.rmtree(root, ignore_errors=True)
        root.mkdir(parents=True, exist_ok=True)
        env = {
            "PATH": "/usr/bin:/bin",
            "LANG": "C.UTF-8",
            "HOME": str(root),
            "XDG_CONFIG_HOME": str(root / ".config"),
            "XDG_RUNTIME_DIR": str(FAKE_XDG_RUNTIME),
        }
        result = subprocess.run(
            [RUST_BIN, *args],
            env=env,
            cwd=root,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=10,
        )
        return result, root / ".config" / "rustype" / "config"

    def test_engine_default_file_loads_to_python_defaults(self):
        result, engine_file = self.run_engine(["--write-config"])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(engine_file.exists())
        put(mod.CONFIG_FILE, engine_file.read_bytes())
        self.assertEqual(snapshot(load_cm()), DEFAULTS)

    def test_engine_default_file_uses_only_keys_the_app_manages(self):
        result, engine_file = self.run_engine(["--write-config"])
        self.assertEqual(result.returncode, 0, result.stderr)
        keys = {mod.config_key_value(l)[0] for l in engine_file.read_text(encoding="utf-8").splitlines()
                if mod.config_key_value(l)}
        self.assertTrue(keys <= set(mod.TOP_KEYS), keys - set(mod.TOP_KEYS))

    def test_saving_over_the_engine_default_file_keeps_its_values_and_comments(self):
        result, engine_file = self.run_engine(["--write-config"])
        self.assertEqual(result.returncode, 0, result.stderr)
        put(mod.CONFIG_FILE, engine_file.read_bytes())
        before = snapshot(load_cm())
        load_cm().save()
        self.assertEqual(snapshot(load_cm()), before)
        self.assertIn("# Rustype: bộ gõ tiếng Việt Telex.", mod.CONFIG_FILE.read_text(encoding="utf-8"))

    def test_write_config_does_not_overwrite_an_existing_file(self):
        self.run_engine(["--write-config"])
        result, _ = self.run_engine(["--write-config"], fresh=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("giữ nguyên", result.stdout)


class WindowModelTests(SettingsTestCase):
    def test_window_shows_saved_mode_and_method(self):
        put(mod.CONFIG_FILE, "mode = preedit\ninput_method = vni\n")
        win = open_window()
        self.assertEqual((win.row_mode.get_selected(), win.row_method.get_selected()), (1, 1))

    def test_direct_and_auto_modes_are_selected(self):
        put(mod.CONFIG_FILE, "mode = direct\n")
        self.assertEqual(open_window().row_mode.get_selected(), 2)
        put(mod.CONFIG_FILE, "mode = auto\n")
        self.assertEqual(open_window().row_mode.get_selected(), 0)

    def test_vni_in_any_case_is_selected(self):
        put(mod.CONFIG_FILE, "input_method = VNI\n")
        self.assertEqual(open_window().row_method.get_selected(), 1)

    def test_window_shows_auto_for_global_mode_off(self):
        # The window shows the same normalised mode as the engine (config.rs:266-270).
        put(mod.CONFIG_FILE, "mode = off\n")
        self.assertEqual(open_window().row_mode.get_selected(), 0)

    def test_window_recognises_mode_in_any_case(self):
        # The engine lowercases the mode value (config.rs:232).
        put(mod.CONFIG_FILE, "mode = Preedit\n")
        self.assertEqual(open_window().row_mode.get_selected(), 1)

    def test_window_shows_telex_for_unknown_input_method(self):
        # The engine uses Telex for any value other than vni (config.rs:273-277).
        put(mod.CONFIG_FILE, "input_method = bogus\n")
        self.assertEqual(open_window().row_method.get_selected(), 0)

    def test_toggle_switch_follows_the_toggle_shortcut(self):
        put(mod.CONFIG_FILE, "toggle_shortcut = off\n")
        self.assertFalse(open_window().sw_toggle.get_active())
        put(mod.CONFIG_FILE, "toggle_shortcut = ctrl_shift\n")
        self.assertTrue(open_window().sw_toggle.get_active())

    def test_switch_change_is_saved(self):
        win = open_window()
        win.sw_simple.set_active(True)
        self.assertTrue(load_cm().simple_telex)

    def test_method_and_mode_changes_are_saved(self):
        win = open_window()
        win.row_method.set_selected(1)
        win.row_mode.set_selected(2)
        reloaded = load_cm()
        self.assertEqual((reloaded.input_method, reloaded.mode), ("vni", "direct"))

    def test_every_switch_is_wired_to_its_option(self):
        switches = (
            ("sw_modern", "modern_orthography"),
            ("sw_english", "auto_restore_english"),
            ("sw_remember", "remember_state_per_app"),
            ("sw_free_tone", "free_tone_order"),
            ("sw_simple", "simple_telex"),
            ("sw_quick", "quick_telex"),
            ("sw_capitalize", "auto_capitalize"),
            ("sw_macro", "enable_macro"),
        )
        for switch_name, attr in switches:
            with self.subTest(option=attr):
                reset_config_dir()
                win = open_window()
                getattr(win, switch_name).set_active(not DEFAULTS[attr])
                self.assertEqual(getattr(load_cm(), attr), not DEFAULTS[attr])

    def test_off_app_add_and_delete_through_the_ui(self):
        win = open_window()
        submit_dialog(win.on_add_off_clicked, "Org.GNOME.Terminal")
        self.assertEqual(load_cm().app_modes, {"org.gnome.terminal": "off"})
        self.assertEqual(win.grp_off.titles(), ["org.gnome.terminal"])
        delete_button("org.gnome.terminal").emit("clicked")
        self.assertEqual(load_cm().app_modes, {})
        self.assertEqual(win.grp_off.titles(), [])

    def test_deleting_an_off_app_keeps_other_modes(self):
        put(
            mod.CONFIG_FILE,
            '[app_modes]\n"kitty" = preedit\n"org.gnome.Terminal" = off\n"firefox" = direct\n',
        )
        win = open_window()
        delete_button("org.gnome.Terminal").emit("clicked")
        self.assertEqual(load_cm().app_modes, {"kitty": "preedit", "firefox": "direct"})
        self.assertEqual(win.grp_off.titles(), [])

    def test_invalid_app_id_is_refused(self):
        win = open_window()
        submit_dialog(win.on_add_off_clicked, "bad app")
        self.assertEqual(load_cm().app_modes, {})
        self.assertIn("Mã ứng dụng không hợp lệ", win.toasts)

    def test_adding_an_off_app_refuses_a_mode_under_a_different_case(self):
        # The engine lowercases app ids into one map, last line wins (config.rs:257-258).
        put(mod.CONFIG_FILE, '[app_modes]\n"org.gnome.Terminal" = preedit\n')
        win = open_window()
        submit_dialog(win.on_add_off_clicked, "org.gnome.terminal")
        same_app = [k for k in load_cm().app_modes if k.lower() == "org.gnome.terminal"]
        self.assertEqual(same_app, ["org.gnome.Terminal"])

    def test_dotted_app_mode_off_is_listed_in_the_off_list(self):
        # The engine honours app_mode.<app> (config.rs:333-336), so the off list must show it.
        put(mod.CONFIG_FILE, "app_mode.kitty = off\n")
        self.assertEqual(open_window().grp_off.titles(), ["kitty"])


class FixedBugTests(SettingsTestCase):
    def test_mode_outside_the_three_values_becomes_auto(self):
        for value, expected in (("bogus", "auto"), ("off", "auto"), ("Direct", "direct"), ("PREEDIT", "preedit")):
            with self.subTest(value=value):
                put(mod.CONFIG_FILE, f"mode = {value}\n")
                self.assertEqual(load_cm().mode, expected)

    def test_input_method_is_vni_only_for_vni(self):
        for value, expected in (("VNI", "vni"), ("vni", "vni"), ("bogus", "telex"), ("", "telex")):
            with self.subTest(value=value):
                put(mod.CONFIG_FILE, f"input_method = {value}\n")
                self.assertEqual(load_cm().input_method, expected)

    def test_bool_spellings_follow_parse_bool(self):
        for spelling in ("true", "yes", "on", "1", "TRUE"):
            with self.subTest(spelling=spelling):
                put(mod.CONFIG_FILE, f"enable_macro = {spelling}\n")
                self.assertTrue(load_cm().enable_macro)
        for spelling in ("false", "no", "off", "0"):
            with self.subTest(spelling=spelling):
                put(mod.CONFIG_FILE, f"free_tone_order = {spelling}\n")
                self.assertFalse(load_cm().free_tone_order)
        put(mod.CONFIG_FILE, "enable_macro = maybe\nfree_tone_order = maybe\n")
        cm = load_cm()
        self.assertFalse(cm.enable_macro)
        self.assertTrue(cm.free_tone_order)

    def test_add_macro_row_appears_in_the_list(self):
        win = open_window()
        submit_dialog(win.on_add_macro_clicked, "vn", "Việt Nam")
        self.assertIn("vn", win.grp_macro_list.titles())

    def test_add_word_row_appears_in_the_list(self):
        win = open_window()
        submit_dialog(win.on_add_word_clicked, "github")
        self.assertEqual(win.grp_words.titles(), ["github"])

    def test_add_macro_removes_the_same_key_from_the_macro_file(self):
        put(mod.MACRO_FILE, "vn: Việt Nam\nhn: Hà Nội\n")
        win = open_window()
        submit_dialog(win.on_add_macro_clicked, "vn", "Việt Nam Mới")
        text = mod.MACRO_FILE.read_text(encoding="utf-8")
        self.assertNotIn("vn:", text)
        self.assertIn("hn: Hà Nội", text)
        self.assertEqual(load_cm().macros, {"vn": "Việt Nam Mới", "hn": "Hà Nội"})

    def test_add_macro_refuses_a_value_with_both_quote_and_hash(self):
        win = open_window()
        submit_dialog(win.on_add_macro_clicked, "x", 'a "b" # c')
        self.assertEqual(load_cm().macros, {})
        self.assertEqual(win.cfg.macros, {})
        self.assertIn("dấu", win.toasts[-1])

    def test_words_file_comma_line_stays_out_of_config_and_file(self):
        put(mod.WORDS_FILE, "a,b\ngithub\n")
        cm = load_cm()
        self.assertEqual(cm.custom_words, ["a,b", "github"])
        cm.simple_telex = True
        cm.save()
        config_text = mod.CONFIG_FILE.read_text(encoding="utf-8")
        self.assertIn("custom_words = github", config_text)
        self.assertNotIn("a,b", config_text)
        self.assertEqual(mod.WORDS_FILE.read_text(encoding="utf-8"), "a,b\ngithub\n")

    def test_already_off_app_is_not_duplicated(self):
        put(mod.CONFIG_FILE, '[app_modes]\n"kitty" = off\n')
        win = open_window()
        submit_dialog(win.on_add_off_clicked, "Kitty")
        self.assertEqual(list(load_cm().app_modes), ["kitty"])
        self.assertEqual(win.grp_off.titles(), ["kitty"])

    def test_dotted_off_app_is_gone_after_delete_and_reload(self):
        put(mod.CONFIG_FILE, "app_mode.org.gnome.Terminal = off\n")
        win = open_window()
        self.assertEqual(win.grp_off.titles(), ["org.gnome.terminal"])
        delete_button("org.gnome.terminal").emit("clicked")
        self.assertEqual(load_cm().app_modes, {})
        self.assertNotIn("org.gnome.Terminal", mod.CONFIG_FILE.read_text(encoding="utf-8"))
        self.assertEqual(open_window().grp_off.titles(), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)

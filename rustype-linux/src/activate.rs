//! `rustype-linux --activate` and `--deactivate`: the per-user steps that make an installed
//! package usable at once: the GNOME input source, the focus extension, the config file, and a
//! restart of the IBus unit when it runs another binary. Std only, and idempotent.
//!
//! Every external command is bounded by a time limit and gets no stdin. The commands never prompt.

use crate::config;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const UNIT: &str = "org.freedesktop.IBus.session.GNOME.service";
const EXTENSION: &str = "rustype-focus@namtr.dev";
const SCHEMA: &str = "org.gnome.desktop.input-sources";
const STAMP_NAME: &str = "activated";
/// Upper bound for one external command; the package scripts add their own outer bound.
const CHILD_LIMIT: Duration = Duration::from_secs(8);

/// An input source: (type, id), as in the `sources` key, for example `("ibus", "rustype")`.
pub type Source = (String, String);

/// The files this module reads and writes.
pub struct Paths {
    /// `~/.config/rustype/config`; created when missing, never overwritten.
    pub config: PathBuf,
    /// `~/.config/rustype/activated`; while it exists, activation does not add the source again.
    pub stamp: PathBuf,
}

impl Paths {
    /// The paths under the user's configuration directory, or None when HOME is unknown.
    pub fn resolve() -> Option<Paths> {
        let config = config::path()?;
        let stamp = config.parent()?.join(STAMP_NAME);
        Some(Paths { config, stamp })
    }
}

fn is_rustype(s: &Source) -> bool {
    s.0 == "ibus" && s.1 == "rustype"
}

/// Parses the output of `gsettings get ... sources`: `@a(ss) []`, `[]`, or a list of quoted pairs
/// such as `[('xkb', 'us'), ('ibus', 'bopomofo')]`. Returns None for any other text, so a value
/// this code does not fully understand is never written back.
pub fn parse_sources(text: &str) -> Option<Vec<Source>> {
    let mut s = text.trim();
    if let Some(rest) = s.strip_prefix('@') {
        let (ty, rest) = rest.split_once(char::is_whitespace)?;
        if ty != "a(ss)" {
            return None;
        }
        s = rest.trim_start();
    }
    let mut c = Cursor {
        chars: s.chars().collect(),
        pos: 0,
    };
    c.expect('[')?;
    let mut out = Vec::new();
    if c.eat(']') {
        return c.finish().then_some(out);
    }
    loop {
        c.expect('(')?;
        let a = c.string()?;
        c.expect(',')?;
        let b = c.string()?;
        c.expect(')')?;
        out.push((a, b));
        if c.eat(',') {
            continue;
        }
        c.expect(']')?;
        return c.finish().then_some(out);
    }
}

/// Formats a list the way `gsettings set` accepts it: `[('xkb', 'us')]`, or `[]` when empty.
pub fn format_sources(list: &[Source]) -> String {
    let items: Vec<String> = list
        .iter()
        .map(|(a, b)| format!("({}, {})", quote(a), quote(b)))
        .collect();
    format!("[{}]", items.join(", "))
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// The list with Rustype inserted at index 1, or at index 0 when the list is empty. A list that
/// already names Rustype is returned unchanged.
pub fn with_rustype(mut list: Vec<Source>) -> Vec<Source> {
    if !list.iter().any(is_rustype) {
        let at = 1.min(list.len());
        list.insert(at, ("ibus".to_string(), "rustype".to_string()));
    }
    list
}

/// The list without any Rustype entry.
pub fn without_rustype(mut list: Vec<Source>) -> Vec<Source> {
    list.retain(|s| !is_rustype(s));
    list
}

/// Whether the unit must be restarted: its main process must run exactly the binary that is
/// running this command. A missing main process, an unreadable exe, or a path marked "(deleted)"
/// after an upgrade all count as different.
pub fn needs_restart(unit_exe: Option<&Path>, running: &Path) -> bool {
    unit_exe != Some(running)
}

/// A MainPID line from `systemctl show -p MainPID --value`; zero or garbage gives None.
pub fn parse_pid(text: &str) -> Option<i32> {
    text.trim().parse::<i32>().ok().filter(|p| *p > 0)
}

/// A read position in the characters of a gsettings value.
struct Cursor {
    chars: Vec<char>,
    pos: usize,
}

impl Cursor {
    fn skip_ws(&mut self) {
        while self.chars.get(self.pos).is_some_and(|c| c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, want: char) -> bool {
        self.skip_ws();
        if self.chars.get(self.pos) == Some(&want) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, want: char) -> Option<()> {
        self.eat(want).then_some(())
    }

    /// A string in single or double quotes, where only `\\`, `\'` and `\"` are escapes.
    fn string(&mut self) -> Option<String> {
        self.skip_ws();
        let quote = *self.chars.get(self.pos)?;
        if quote != '\'' && quote != '"' {
            return None;
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            let c = *self.chars.get(self.pos)?;
            self.pos += 1;
            if c == quote {
                return Some(out);
            }
            if c == '\\' {
                let e = *self.chars.get(self.pos)?;
                self.pos += 1;
                if e != '\\' && e != '\'' && e != '"' {
                    return None;
                }
                out.push(e);
            } else {
                out.push(c);
            }
        }
    }

    /// True when only whitespace is left.
    fn finish(&mut self) -> bool {
        self.skip_ws();
        self.pos == self.chars.len()
    }
}

/// The external effects of activation, so the decisions can be tested without a session.
pub trait Host {
    /// `systemctl --user cat UNIT` succeeds: this is a GNOME user session with the unit.
    fn unit_exists(&self) -> bool;
    /// The current `sources` value, or None when gsettings fails.
    fn gsettings_get(&self) -> Option<String>;
    /// Writes the `sources` value; false when gsettings fails.
    fn gsettings_set(&self, value: &str) -> bool;
    /// Enables the focus extension. Failure is ignored.
    fn enable_extension(&self);
    fn daemon_reload(&self);
    /// The exe of the unit's MainPID, or None when there is no such process.
    fn unit_exe(&self) -> Option<PathBuf>;
    fn restart_unit(&self);
}

/// Adds Rustype to the input sources unless it is there. Returns false when the sources could not
/// be read or written, in which case the caller must not write the stamp.
fn add_source(host: &impl Host) -> bool {
    let Some(text) = host.gsettings_get() else {
        return false;
    };
    let Some(list) = parse_sources(&text) else {
        eprintln!("rustype-linux: cannot read the GNOME input sources: {text}");
        return false;
    };
    if list.iter().any(is_rustype) {
        return true;
    }
    host.gsettings_set(&format_sources(&with_rustype(list)))
}

fn write_stamp(stamp: &Path) -> std::io::Result<()> {
    if let Some(dir) = stamp.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(stamp, b"")
}

/// `--activate [--restart]`.
pub fn activate_with(host: &impl Host, paths: &Paths, running: &Path, restart: bool) {
    if !host.unit_exists() {
        return;
    }
    if let Err(e) = config::create_default(&paths.config) {
        eprintln!("rustype-linux: {}: {e}", paths.config.display());
    }
    if !paths.stamp.exists() && add_source(host) {
        host.enable_extension();
        if let Err(e) = write_stamp(&paths.stamp) {
            eprintln!("rustype-linux: {}: {e}", paths.stamp.display());
        }
    }
    if restart {
        host.daemon_reload();
        if needs_restart(host.unit_exe().as_deref(), running) {
            host.restart_unit();
        }
    }
}

/// `--deactivate`: removes Rustype from the input sources and clears the stamp. The config stays.
pub fn deactivate_with(host: &impl Host, paths: &Paths) {
    if let Some(list) = host.gsettings_get().and_then(|text| parse_sources(&text)) {
        if list.iter().any(is_rustype) {
            host.gsettings_set(&format_sources(&without_rustype(list)));
        }
    }
    let _ = std::fs::remove_file(&paths.stamp);
}

/// The real host: every command runs with a time limit and no stdin.
struct System;

struct Done {
    ok: bool,
    stdout: String,
}

fn run(prog: &str, args: &[&str]) -> Option<Done> {
    let mut child = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + CHILD_LIMIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_string(&mut stdout);
                }
                return Some(Done {
                    ok: status.success(),
                    stdout,
                });
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

impl Host for System {
    fn unit_exists(&self) -> bool {
        run("systemctl", &["--user", "cat", UNIT]).is_some_and(|d| d.ok)
    }

    fn gsettings_get(&self) -> Option<String> {
        let d = run("gsettings", &["get", SCHEMA, "sources"])?;
        d.ok.then(|| d.stdout.trim().to_string())
    }

    fn gsettings_set(&self, value: &str) -> bool {
        run("gsettings", &["set", SCHEMA, "sources", value]).is_some_and(|d| d.ok)
    }

    fn enable_extension(&self) {
        let _ = run("gnome-extensions", &["enable", EXTENSION]);
    }

    fn daemon_reload(&self) {
        let _ = run("systemctl", &["--user", "daemon-reload"]);
    }

    fn unit_exe(&self) -> Option<PathBuf> {
        let d = run(
            "systemctl",
            &["--user", "show", "-p", "MainPID", "--value", UNIT],
        )?;
        let pid = parse_pid(&d.stdout)?;
        std::fs::read_link(format!("/proc/{pid}/exe")).ok()
    }

    fn restart_unit(&self) {
        let _ = run("systemctl", &["--user", "--no-block", "restart", UNIT]);
    }
}

/// Entry point of `--activate [--restart]`. Always returns; the caller exits with status 0.
pub fn activate(restart: bool) {
    let Some(paths) = Paths::resolve() else {
        return;
    };
    let Ok(running) = std::env::current_exe() else {
        return;
    };
    activate_with(&System, &paths, &running, restart);
}

/// Entry point of `--deactivate`. Always returns; the caller exits with status 0.
pub fn deactivate() {
    let Some(paths) = Paths::resolve() else {
        return;
    };
    deactivate_with(&System, &paths);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn src(a: &str, b: &str) -> Source {
        (a.to_string(), b.to_string())
    }

    #[test]
    fn parses_gsettings_values() {
        assert_eq!(parse_sources("@a(ss) []"), Some(vec![]));
        assert_eq!(parse_sources("[]\n"), Some(vec![]));
        assert_eq!(
            parse_sources("[('xkb', 'us')]"),
            Some(vec![src("xkb", "us")])
        );
        assert_eq!(
            parse_sources("[('xkb', 'us'), ('ibus', 'bopomofo')]"),
            Some(vec![src("xkb", "us"), src("ibus", "bopomofo")])
        );
        assert_eq!(
            parse_sources(r#"[("xkb", "a'b"), ('xkb', 'c\'d\\e')]"#),
            Some(vec![src("xkb", "a'b"), src("xkb", "c'd\\e")])
        );
    }

    #[test]
    fn rejects_values_it_does_not_understand() {
        for bad in [
            "",
            "[('xkb')]",
            "[('xkb', 'us'),]",
            "[('xkb', 'us')] extra",
            "@s 'x'",
            "@a(ss)",
            "[('xkb', 'u\\nx')]",
            "[('xkb', 'us'",
            "('xkb', 'us')",
        ] {
            assert_eq!(parse_sources(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn formats_and_round_trips() {
        let list = vec![src("xkb", "us"), src("ibus", "a'b\\c")];
        let text = format_sources(&list);
        assert_eq!(text, "[('xkb', 'us'), ('ibus', 'a\\'b\\\\c')]");
        assert_eq!(parse_sources(&text), Some(list));
        assert_eq!(format_sources(&[]), "[]");
    }

    #[test]
    fn inserts_at_index_one_or_zero() {
        assert_eq!(with_rustype(vec![]), vec![src("ibus", "rustype")]);
        assert_eq!(
            with_rustype(vec![src("xkb", "us")]),
            vec![src("xkb", "us"), src("ibus", "rustype")]
        );
        assert_eq!(
            with_rustype(vec![src("xkb", "us"), src("ibus", "bopomofo")]),
            vec![
                src("xkb", "us"),
                src("ibus", "rustype"),
                src("ibus", "bopomofo")
            ]
        );
        let present = vec![src("xkb", "us"), src("ibus", "rustype")];
        assert_eq!(with_rustype(present.clone()), present);
    }

    #[test]
    fn removes_every_rustype_entry() {
        assert_eq!(
            without_rustype(vec![
                src("ibus", "rustype"),
                src("xkb", "us"),
                src("ibus", "rustype"),
            ]),
            vec![src("xkb", "us")]
        );
    }

    #[test]
    fn restarts_only_when_the_exe_differs() {
        let running = Path::new("/usr/libexec/rustype-linux");
        assert!(!needs_restart(Some(running), running));
        assert!(needs_restart(
            Some(Path::new("/usr/libexec/rustype-linux (deleted)")),
            running
        ));
        assert!(needs_restart(
            Some(Path::new("/usr/bin/ibus-daemon")),
            running
        ));
        assert!(needs_restart(None, running));
    }

    #[test]
    fn parses_main_pid() {
        assert_eq!(parse_pid("1234\n"), Some(1234));
        assert_eq!(parse_pid("0\n"), None);
        assert_eq!(parse_pid("\n"), None);
        assert_eq!(parse_pid("abc"), None);
    }

    /// Records every call; `sources` is what gsettings get returns (None: it fails).
    struct Fake {
        unit: bool,
        sources: RefCell<Option<String>>,
        set_ok: bool,
        exe: Option<PathBuf>,
        calls: RefCell<Vec<String>>,
    }

    impl Fake {
        fn new(sources: Option<&str>) -> Fake {
            Fake {
                unit: true,
                sources: RefCell::new(sources.map(str::to_string)),
                set_ok: true,
                exe: None,
                calls: RefCell::new(Vec::new()),
            }
        }

        fn log(&self, what: &str) {
            self.calls.borrow_mut().push(what.to_string());
        }

        fn count(&self, prefix: &str) -> usize {
            self.calls
                .borrow()
                .iter()
                .filter(|c| c.starts_with(prefix))
                .count()
        }
    }

    impl Host for Fake {
        fn unit_exists(&self) -> bool {
            self.unit
        }
        fn gsettings_get(&self) -> Option<String> {
            self.log("get");
            self.sources.borrow().clone()
        }
        fn gsettings_set(&self, value: &str) -> bool {
            self.log(&format!("set {value}"));
            if self.set_ok {
                *self.sources.borrow_mut() = Some(value.to_string());
            }
            self.set_ok
        }
        fn enable_extension(&self) {
            self.log("enable");
        }
        fn daemon_reload(&self) {
            self.log("reload");
        }
        fn unit_exe(&self) -> Option<PathBuf> {
            self.exe.clone()
        }
        fn restart_unit(&self) {
            self.log("restart");
        }
    }

    /// A private config directory for one test, removed when dropped.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> TestDir {
            let p = std::env::temp_dir().join(format!(
                "rustype-activate-test-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TestDir(p)
        }

        fn paths(&self) -> Paths {
            let config = self.0.join("rustype").join("config");
            let stamp = self.0.join("rustype").join(STAMP_NAME);
            Paths { config, stamp }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const RUNNING: &str = "/usr/libexec/rustype-linux";

    #[test]
    fn first_activation_adds_source_once_and_stamps() {
        let dir = TestDir::new("first");
        let paths = dir.paths();
        let fake = Fake::new(Some("[('xkb', 'us')]"));
        activate_with(&fake, &paths, Path::new(RUNNING), false);
        assert_eq!(
            fake.sources.borrow().as_deref(),
            Some("[('xkb', 'us'), ('ibus', 'rustype')]")
        );
        assert_eq!(fake.count("set"), 1);
        assert_eq!(fake.count("enable"), 1);
        assert!(paths.stamp.exists());
        assert!(paths.config.exists());

        // A second run changes nothing: the source is there and the stamp exists.
        activate_with(&fake, &paths, Path::new(RUNNING), false);
        assert_eq!(fake.count("set"), 1);
        assert_eq!(fake.count("enable"), 1);
    }

    #[test]
    fn stamp_stops_the_source_being_added_back() {
        let dir = TestDir::new("stamp");
        let paths = dir.paths();
        let fake = Fake::new(Some("[('xkb', 'us')]"));
        activate_with(&fake, &paths, Path::new(RUNNING), false);
        // The user removes Rustype by hand.
        *fake.sources.borrow_mut() = Some("[('xkb', 'us')]".to_string());
        let before = fake.count("set");
        activate_with(&fake, &paths, Path::new(RUNNING), false);
        assert_eq!(fake.count("set"), before);
        assert_eq!(fake.sources.borrow().as_deref(), Some("[('xkb', 'us')]"));
    }

    #[test]
    fn empty_and_multi_entry_lists_insert_at_the_right_index() {
        let dir = TestDir::new("index");
        let fake = Fake::new(Some("@a(ss) []"));
        activate_with(&fake, &dir.paths(), Path::new(RUNNING), false);
        assert_eq!(
            fake.sources.borrow().as_deref(),
            Some("[('ibus', 'rustype')]")
        );

        let dir = TestDir::new("index2");
        let fake = Fake::new(Some("[('xkb', 'us'), ('ibus', 'bopomofo')]"));
        activate_with(&fake, &dir.paths(), Path::new(RUNNING), false);
        assert_eq!(
            fake.sources.borrow().as_deref(),
            Some("[('xkb', 'us'), ('ibus', 'rustype'), ('ibus', 'bopomofo')]")
        );
    }

    #[test]
    fn present_source_is_not_written_again_but_is_stamped() {
        let dir = TestDir::new("present");
        let fake = Fake::new(Some("[('xkb', 'us'), ('ibus', 'rustype')]"));
        activate_with(&fake, &dir.paths(), Path::new(RUNNING), false);
        assert_eq!(fake.count("set"), 0);
        assert!(dir.paths().stamp.exists());
    }

    #[test]
    fn failures_leave_no_stamp_so_the_next_login_retries() {
        let dir = TestDir::new("fail-get");
        let fake = Fake::new(None);
        activate_with(&fake, &dir.paths(), Path::new(RUNNING), false);
        assert!(!dir.paths().stamp.exists());
        assert_eq!(fake.count("set"), 0);
        assert_eq!(fake.count("enable"), 0);

        let dir = TestDir::new("fail-parse");
        let fake = Fake::new(Some("@s 'weird'"));
        activate_with(&fake, &dir.paths(), Path::new(RUNNING), false);
        assert!(!dir.paths().stamp.exists());
        assert_eq!(fake.count("set"), 0);

        let dir = TestDir::new("fail-set");
        let mut fake = Fake::new(Some("[('xkb', 'us')]"));
        fake.set_ok = false;
        activate_with(&fake, &dir.paths(), Path::new(RUNNING), false);
        assert!(!dir.paths().stamp.exists());
    }

    #[test]
    fn no_unit_means_no_work_at_all() {
        let dir = TestDir::new("no-unit");
        let mut fake = Fake::new(Some("[]"));
        fake.unit = false;
        activate_with(&fake, &dir.paths(), Path::new(RUNNING), true);
        assert!(fake.calls.borrow().is_empty());
        assert!(!dir.paths().config.exists());
    }

    #[test]
    fn existing_config_is_kept() {
        let dir = TestDir::new("config");
        let paths = dir.paths();
        std::fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        std::fs::write(&paths.config, "mode = preedit\n").unwrap();
        let fake = Fake::new(Some("[]"));
        activate_with(&fake, &paths, Path::new(RUNNING), false);
        assert_eq!(
            std::fs::read_to_string(&paths.config).unwrap(),
            "mode = preedit\n"
        );
    }

    #[test]
    fn restart_only_when_the_unit_runs_another_exe() {
        let dir = TestDir::new("restart");
        let paths = dir.paths();

        let mut same = Fake::new(Some("[]"));
        same.exe = Some(PathBuf::from(RUNNING));
        activate_with(&same, &paths, Path::new(RUNNING), true);
        assert_eq!(same.count("reload"), 1);
        assert_eq!(same.count("restart"), 0);

        let mut upgraded = Fake::new(Some("[]"));
        upgraded.exe = Some(PathBuf::from("/usr/libexec/rustype-linux (deleted)"));
        activate_with(&upgraded, &paths, Path::new(RUNNING), true);
        assert_eq!(upgraded.count("restart"), 1);

        let mut ibus = Fake::new(Some("[]"));
        ibus.exe = Some(PathBuf::from("/usr/bin/ibus-daemon"));
        activate_with(&ibus, &paths, Path::new(RUNNING), true);
        assert_eq!(ibus.count("restart"), 1);

        // Without --restart nothing is reloaded or restarted.
        let mut plain = Fake::new(Some("[]"));
        plain.exe = Some(PathBuf::from("/usr/bin/ibus-daemon"));
        activate_with(&plain, &paths, Path::new(RUNNING), false);
        assert_eq!(plain.count("reload"), 0);
        assert_eq!(plain.count("restart"), 0);
    }

    #[test]
    fn deactivate_removes_only_rustype_and_the_stamp() {
        let dir = TestDir::new("deactivate");
        let paths = dir.paths();
        let fake = Fake::new(Some("[('xkb', 'us')]"));
        activate_with(&fake, &paths, Path::new(RUNNING), false);
        assert!(paths.stamp.exists());

        deactivate_with(&fake, &paths);
        assert_eq!(fake.sources.borrow().as_deref(), Some("[('xkb', 'us')]"));
        assert!(!paths.stamp.exists());
        assert!(paths.config.exists());

        // Nothing to remove: no write.
        let sets = fake.count("set");
        deactivate_with(&fake, &paths);
        assert_eq!(fake.count("set"), sets);
    }
}

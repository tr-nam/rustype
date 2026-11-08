//! The IBus address file, as ibus-daemon writes it (FACTS B8), and the guard that refuses to
//! overwrite the file of a live IBus daemon or another rustype-linux.

use crate::sys;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// `$IBUS_ADDRESS_FILE`, else `$XDG_CONFIG_HOME/ibus/bus/<machine>-unix-<display>`.
pub fn resolve_path(
    address_file_env: Option<OsString>,
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
    machine_id: &str,
    display: &str,
) -> PathBuf {
    if let Some(f) = address_file_env.filter(|f| !f.is_empty()) {
        return PathBuf::from(f);
    }
    let base = xdg_config_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("ibus")
        .join("bus")
        .join(format!("{machine_id}-unix-{display}"))
}

/// Machine id as ibus computes it: /var/lib/dbus/machine-id, /etc/machine-id, else a fixed name.
pub fn machine_id() -> String {
    for p in ["/var/lib/dbus/machine-id", "/etc/machine-id"] {
        if let Ok(s) = std::fs::read_to_string(p) {
            let s = s.trim();
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    "machine-id".to_string()
}

/// WAYLAND_DISPLAY if set, else DISPLAY, as ibus does.
pub fn display_name(wayland: Option<OsString>, x11: Option<OsString>) -> String {
    wayland
        .or(x11)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The `IBUS_DAEMON_PID=` value, if the file has one.
pub fn parse_pid(text: &str) -> Option<i32> {
    text.lines()
        .find_map(|l| l.strip_prefix("IBUS_DAEMON_PID="))
        .and_then(|v| v.trim().parse().ok())
}

/// Refuses (Err) when the file names a live process other than `my_pid`, unless `replace`.
/// A missing file, a file naming us, or a file naming a dead process is fine.
pub fn guard(path: &Path, my_pid: i32, replace: bool) -> io::Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let Some(pid) = parse_pid(&text) else {
        return Ok(());
    };
    if pid == my_pid || !sys::pid_alive(pid) {
        return Ok(());
    }
    if replace {
        eprintln!(
            "rustype-linux: --replace: taking over {} from live process {pid}",
            path.display()
        );
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::AddrInUse,
        format!(
            "{} names live process {pid}; another IBus daemon or rustype-linux is running. \
             Stop it, or pass --replace to take over the file.",
            path.display()
        ),
    ))
}

/// Writes the address file (atomic rename) in the layout ibus-daemon uses.
pub fn write(path: &Path, address: &str, pid: i32) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = format!(
        "# This file is created by rustype-linux, in the format ibus-daemon uses.\n\
         # If the IBUS_ADDRESS environment variable is set, it will be used rather than this file.\n\
         IBUS_ADDRESS={address}\n\
         IBUS_DAEMON_PID={pid}\n"
    );
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let tmp = path.with_file_name(format!(".{name}.tmp{pid}"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Removes the file only if it still names us, so a successor's file survives our exit.
pub fn remove_if_ours(path: &Path, my_pid: i32) {
    if let Ok(text) = std::fs::read_to_string(path) {
        if parse_pid(&text) == Some(my_pid) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rl-addr-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn path_resolution_honours_overrides() {
        let p = resolve_path(
            Some("/x/addr".into()),
            Some("/cfg".into()),
            Some("/home/u".into()),
            "mid",
            "wayland-0",
        );
        assert_eq!(p, PathBuf::from("/x/addr"));
        let p = resolve_path(
            None,
            Some("/cfg".into()),
            Some("/home/u".into()),
            "mid",
            "wayland-0",
        );
        assert_eq!(p, PathBuf::from("/cfg/ibus/bus/mid-unix-wayland-0"));
        let p = resolve_path(None, None, Some("/home/u".into()), "mid", ":0");
        assert_eq!(p, PathBuf::from("/home/u/.config/ibus/bus/mid-unix-:0"));
        // A relative XDG_CONFIG_HOME is ignored, as the spec says.
        let p = resolve_path(None, Some("rel".into()), Some("/home/u".into()), "m", "d");
        assert_eq!(p, PathBuf::from("/home/u/.config/ibus/bus/m-unix-d"));
    }

    #[test]
    fn pid_line_is_parsed() {
        assert_eq!(
            parse_pid("# c\nIBUS_ADDRESS=unix:path=/a\nIBUS_DAEMON_PID=1234\n"),
            Some(1234)
        );
        assert_eq!(parse_pid("IBUS_ADDRESS=x\n"), None);
    }

    #[test]
    fn missing_file_and_own_pid_are_allowed() {
        let dir = scratch("own");
        let me = std::process::id() as i32;
        let f = dir.join("addr");
        assert!(guard(&f, me, false).is_ok());
        write(&f, "unix:path=/s,guid=00", me).unwrap();
        assert!(guard(&f, me, false).is_ok());
        remove_if_ours(&f, me);
        assert!(!f.exists());
    }

    #[test]
    fn dead_pid_is_allowed() {
        let dir = scratch("dead");
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id() as i32;
        child.wait().unwrap();
        let f = dir.join("addr");
        write(&f, "unix:path=/s,guid=00", pid).unwrap();
        assert!(guard(&f, std::process::id() as i32, false).is_ok());
    }

    #[test]
    fn live_foreign_process_is_refused_unless_replace() {
        let dir = scratch("live");
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id() as i32;
        let f = dir.join("addr");
        write(&f, "unix:path=/s,guid=00", pid).unwrap();
        let me = std::process::id() as i32;

        let err = guard(&f, me, false).expect_err("must refuse a live foreign daemon");
        assert_eq!(err.kind(), io::ErrorKind::AddrInUse);
        assert!(err.to_string().contains(&pid.to_string()));
        assert!(err.to_string().contains("--replace"));
        // The file is untouched by a refused start.
        assert!(std::fs::read_to_string(&f)
            .unwrap()
            .contains(&format!("IBUS_DAEMON_PID={pid}")));

        assert!(guard(&f, me, true).is_ok(), "--replace takes over");
        // remove_if_ours must not delete a file that names someone else.
        remove_if_ours(&f, me);
        assert!(f.exists());

        child.kill().unwrap();
        child.wait().unwrap();
    }
}

//! `~/.config/rustype/config`: a few `key = value` lines, reloaded when the file changes.

use rustype_core::telex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No underline where the app supports Surrounding Text, underline (preedit) elsewhere.
    Auto,
    /// Always underline (preedit). Safest everywhere.
    Preedit,
    /// Never underline: Surrounding Text, or fake BackSpace keys when the app lacks it.
    Direct,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub mode: Mode,
    pub telex: telex::Config,
    pub app_modes: HashMap<String, Mode>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: Mode::Auto,
            telex: telex::Config::default(),
            app_modes: HashMap::new(),
        }
    }
}

impl Config {
    /// Gets the display mode for a specific application, or the global default.
    pub fn mode_for_app(&self, app_id: Option<&str>) -> Mode {
        if let Some(id) = app_id {
            if let Some(m) = self.app_modes.get(id) {
                return *m;
            }
        }
        self.mode
    }
}

pub const DEFAULT_FILE: &str = "\
# Rustype: bộ gõ tiếng Việt Telex.
# Sửa xong thì bấm sang ô gõ khác là có hiệu lực, không cần khởi động lại.

# auto    : không gạch chân ở ứng dụng thường, gạch chân trong terminal (khuyên dùng)
# preedit : luôn gạch chân
# direct  : không bao giờ gạch chân (terminal trên Wayland có thể bị lặp chữ)
mode = auto

# false: hòa, khỏe, thúy   true: hoà, khoẻ, thuý
modern_orthography = false

# Tự trả lại từ tiếng Anh khi gõ window, software, user...
auto_restore_english = true

# Cấu hình chế độ riêng cho từng ứng dụng (tuỳ chọn):
# [app_modes]
# \"org.gnome.Ptyxis\" = preedit
# \"kitty\" = preedit
# \"google-chrome\" = auto
";

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("rustype").join("config"))
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn parse_mode(v: &str) -> Mode {
    match v {
        "preedit" => Mode::Preedit,
        "direct" => Mode::Direct,
        _ => Mode::Auto,
    }
}

pub fn parse(text: &str) -> Config {
    let mut cfg = Config::default();
    let mut section = "";
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim().trim_matches('"'), v.trim().trim_matches('"'));
        if section == "app_modes" {
            cfg.app_modes.insert(k.to_string(), parse_mode(v));
            continue;
        }
        match k {
            "mode" => cfg.mode = parse_mode(v),
            "modern_orthography" => {
                if let Some(b) = parse_bool(v) {
                    cfg.telex.modern_orthography = b;
                }
            }
            "auto_restore_english" => {
                if let Some(b) = parse_bool(v) {
                    cfg.telex.auto_restore_english = b;
                }
            }
            other if other.starts_with("app_mode.") => {
                let app = other.strip_prefix("app_mode.").unwrap().trim_matches('"');
                cfg.app_modes.insert(app.to_string(), parse_mode(v));
            }
            _ => {}
        }
    }
    cfg
}

/// Holds the current config and re-reads the file only when its mtime changes.
pub struct Watcher {
    path: Option<PathBuf>,
    mtime: Option<SystemTime>,
    pub current: Config,
}

impl Watcher {
    pub fn new() -> Self {
        let mut w = Watcher { path: path(), mtime: None, current: Config::default() };
        w.refresh();
        w
    }

    /// Returns true when the config changed.
    pub fn refresh(&mut self) -> bool {
        let Some(p) = &self.path else { return false };
        let mtime = std::fs::metadata(p).and_then(|m| m.modified()).ok();
        if mtime == self.mtime {
            return false;
        }
        self.mtime = mtime;
        self.current = match std::fs::read_to_string(p) {
            Ok(text) => parse(&text),
            Err(_) => Config::default(),
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_parses_to_defaults() {
        let c = parse(DEFAULT_FILE);
        assert_eq!(c.mode, Mode::Auto);
        assert!(!c.telex.modern_orthography);
        assert!(c.telex.auto_restore_english);
    }

    #[test]
    fn parses_values() {
        let c = parse("mode = preedit\nmodern_orthography = true # kiểu mới\nauto_restore_english=off");
        assert_eq!(c.mode, Mode::Preedit);
        assert!(c.telex.modern_orthography);
        assert!(!c.telex.auto_restore_english);
    }

    #[test]
    fn parses_app_modes() {
        let text = "\
mode = auto
app_mode.kitty = preedit

[app_modes]
org.gnome.Ptyxis = preedit
\"google-chrome\" = auto
";
        let c = parse(text);
        assert_eq!(c.mode, Mode::Auto);
        assert_eq!(c.mode_for_app(Some("org.gnome.Ptyxis")), Mode::Preedit);
        assert_eq!(c.mode_for_app(Some("kitty")), Mode::Preedit);
        assert_eq!(c.mode_for_app(Some("google-chrome")), Mode::Auto);
        assert_eq!(c.mode_for_app(Some("unknown")), Mode::Auto);
        assert_eq!(c.mode_for_app(None), Mode::Auto);
    }
}

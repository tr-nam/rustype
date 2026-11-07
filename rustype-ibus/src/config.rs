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
    pub remember_state_per_app: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: Mode::Auto,
            telex: telex::Config::default(),
            app_modes: HashMap::new(),
            remember_state_per_app: true,
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
            if let Some(base) = id.strip_suffix(".desktop") {
                if let Some(m) = self.app_modes.get(base) {
                    return *m;
                }
            }
            let lower = id.to_lowercase();
            if let Some(m) = self.app_modes.get(&lower) {
                return *m;
            }
            if let Some(base) = lower.strip_suffix(".desktop") {
                if let Some(m) = self.app_modes.get(base) {
                    return *m;
                }
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

# Kiểu gõ: telex | vni
input_method = telex

# Bật gõ tắt (macro) từ file ~/.config/rustype/macro hoặc mục [macros]
enable_macro = false

# Nhớ trạng thái bật/tắt tiếng Việt theo từng ứng dụng (như EVKey)
remember_state_per_app = true

# Từ tiếng Anh bổ sung (không bị bỏ dấu nhầm):
# custom_words = this, test, github, cargo, rust

# Cấu hình chế độ riêng cho từng ứng dụng (tuỳ chọn):
# [app_modes]
# \"org.gnome.Ptyxis\" = preedit
# \"kitty\" = preedit
# \"google-chrome\" = auto

# Bảng gõ tắt (hoặc đặt trong ~/.config/rustype/macro):
# [macros]
# vn = \"Việt Nam\"
# hn = \"Hà Nội\"
";

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("rustype").join("config"))
}

/// Updates `input_method = telex` or `vni` in `~/.config/rustype/config`.
pub fn update_input_method(method: rustype_core::telex::InputMethod) {
    let Some(p) = path() else { return };
    let method_str = match method {
        rustype_core::telex::InputMethod::Telex => "telex",
        rustype_core::telex::InputMethod::Vni => "vni",
    };
    if let Ok(content) = std::fs::read_to_string(&p) {
        let mut lines = Vec::new();
        let mut replaced = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("input_method") && trimmed.contains('=') {
                lines.push(format!("input_method = {method_str}"));
                replaced = true;
            } else {
                lines.push(line.to_string());
            }
        }
        if !replaced {
            lines.push(format!("input_method = {method_str}"));
        }
        let _ = std::fs::write(&p, lines.join("\n") + "\n");
    }
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
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim().trim_matches('"'), v.trim().trim_matches('"'));
        if section == "app_modes" {
            cfg.app_modes.insert(k.to_string(), parse_mode(v));
            continue;
        }
        if section == "macros" {
            cfg.telex.macros.insert(k.to_string(), v.to_string());
            continue;
        }
        match k {
            "mode" => cfg.mode = parse_mode(v),
            "input_method" => {
                cfg.telex.input_method = match v.to_ascii_lowercase().as_str() {
                    "vni" => telex::InputMethod::Vni,
                    _ => telex::InputMethod::Telex,
                };
            }
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
            "remember_state_per_app" => {
                if let Some(b) = parse_bool(v) {
                    cfg.remember_state_per_app = b;
                }
            }
            "enable_macro" => {
                if let Some(b) = parse_bool(v) {
                    cfg.telex.enable_macro = b;
                }
            }
            "custom_words" => {
                for word in v.split(',') {
                    let w = word.trim().trim_matches('"');
                    if !w.is_empty() {
                        cfg.telex.custom_words.push(w.to_string());
                    }
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

pub fn load_extra_files(cfg: &mut Config) {
    if let Some(dir) = path().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
        let macro_file = dir.join("macro");
        if let Ok(content) = std::fs::read_to_string(macro_file) {
            for line in content.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                if let Some((k, v)) = line.split_once(':') {
                    let (k, v) = (k.trim(), v.trim());
                    if !k.is_empty() && !v.is_empty() {
                        cfg.telex.macros.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }
        let words_file = dir.join("words");
        if let Ok(content) = std::fs::read_to_string(words_file) {
            for line in content.lines() {
                let word = line.split('#').next().unwrap_or("").trim();
                if !word.is_empty() {
                    cfg.telex.custom_words.push(word.to_string());
                }
            }
        }
    }
}

/// Holds the current config and re-reads the file only when its mtime changes.
pub struct Watcher {
    path: Option<PathBuf>,
    mtime: Option<SystemTime>,
    pub current: Config,
}

impl Watcher {
    pub fn new() -> Self {
        let mut w = Watcher {
            path: path(),
            mtime: None,
            current: Config::default(),
        };
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
        let mut cfg = match std::fs::read_to_string(p) {
            Ok(text) => parse(&text),
            Err(_) => Config::default(),
        };
        load_extra_files(&mut cfg);
        self.current = cfg;
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
        let c =
            parse("mode = preedit\nmodern_orthography = true # kiểu mới\nauto_restore_english=off");
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
        assert_eq!(c.mode_for_app(Some("google-chrome.desktop")), Mode::Auto);
        assert_eq!(
            c.mode_for_app(Some("org.gnome.Ptyxis.desktop")),
            Mode::Preedit
        );
        assert_eq!(c.mode_for_app(Some("unknown")), Mode::Auto);
        assert_eq!(c.mode_for_app(None), Mode::Auto);
    }

    #[test]
    fn parses_advanced_settings() {
        let text = "\
input_method = vni
remember_state_per_app = false
enable_macro = true
custom_words = \"this, test, github\"

[macros]
vn = \"Việt Nam\"
hn = \"Hà Nội\"
";
        let c = parse(text);
        assert_eq!(c.telex.input_method, telex::InputMethod::Vni);
        assert!(!c.remember_state_per_app);
        assert!(c.telex.enable_macro);
        assert_eq!(c.telex.custom_words, vec!["this", "test", "github"]);
        assert_eq!(
            c.telex.macros.get("vn").map(|s| s.as_str()),
            Some("Việt Nam")
        );
        assert_eq!(c.telex.macros.get("hn").map(|s| s.as_str()), Some("Hà Nội"));
    }
}

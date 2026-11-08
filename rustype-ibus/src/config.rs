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

/// True for a line whose key is exactly `input_method`: `input_method = x` or `input_method=x`.
fn is_input_method_line(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("input_method")
        .is_some_and(|rest| rest.trim_start_matches([' ', '\t']).starts_with('='))
}

/// Updates `input_method = telex` or `vni` in `~/.config/rustype/config`.
pub fn update_input_method(method: rustype_core::telex::InputMethod) {
    if let Some(p) = path() {
        update_input_method_at(&p, method);
    }
}

fn update_input_method_at(p: &std::path::Path, method: rustype_core::telex::InputMethod) {
    let method_str = match method {
        rustype_core::telex::InputMethod::Telex => "telex",
        rustype_core::telex::InputMethod::Vni => "vni",
    };
    // Follow a symlinked config so the link survives the rename below.
    let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let Ok(content) = std::fs::read_to_string(&p) else {
        return;
    };
    let mut lines = Vec::new();
    let mut replaced = false;
    for line in content.lines() {
        if is_input_method_line(line) {
            lines.push(format!("input_method = {method_str}"));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !replaced {
        lines.push(format!("input_method = {method_str}"));
    }
    // Write a sibling temp file, then rename over the config: readers never see a half-written file.
    let Some(name) = p.file_name() else { return };
    let tmp = p.with_file_name(format!(".{}.tmp", name.to_string_lossy()));
    let body = lines.join("\n") + "\n";
    if std::fs::write(&tmp, body).is_ok() && std::fs::rename(&tmp, &p).is_ok() {
        return;
    }
    let _ = std::fs::remove_file(&tmp);
}

/// Cuts a line at the first `#` that is outside double quotes.
fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    for (idx, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            '#' if !in_quotes => return &line[..idx],
            _ => {}
        }
    }
    line
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
        let line = strip_comment(line).trim();
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
            // Lookups in mode_for_app are lowercase, so store the key that way.
            cfg.app_modes.insert(k.to_lowercase(), parse_mode(v));
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
                cfg.app_modes.insert(app.to_lowercase(), parse_mode(v));
            }
            _ => {}
        }
    }
    cfg
}

pub fn load_extra_files(cfg: &mut Config) {
    if let Some(dir) = path().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
        load_extra_files_from(cfg, &dir);
    }
}

fn load_extra_files_from(cfg: &mut Config, dir: &std::path::Path) {
    let macro_file = dir.join("macro");
    if let Ok(content) = std::fs::read_to_string(macro_file) {
        for line in content.lines() {
            let line = strip_comment(line).trim();
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
            let word = strip_comment(line).trim();
            if !word.is_empty() {
                cfg.telex.custom_words.push(word.to_string());
            }
        }
    }
}

/// Holds the current config and re-reads it when the config, `macro` or `words` mtime changes.
pub struct Watcher {
    /// Files whose mtimes are checked: config, then macro and words beside it.
    watched: Vec<PathBuf>,
    dir: Option<PathBuf>,
    stamps: Option<Vec<Option<SystemTime>>>,
    pub current: Config,
}

impl Default for Watcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Watcher {
    pub fn new() -> Self {
        Self::at(path())
    }

    fn at(config_path: Option<PathBuf>) -> Self {
        let dir = config_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let watched = match (&config_path, &dir) {
            (Some(p), Some(d)) => vec![p.clone(), d.join("macro"), d.join("words")],
            _ => Vec::new(),
        };
        let mut w = Watcher {
            watched,
            dir,
            stamps: None,
            current: Config::default(),
        };
        w.refresh();
        w
    }

    /// Returns true when the config changed.
    pub fn refresh(&mut self) -> bool {
        if self.watched.is_empty() {
            return false;
        }
        let now: Vec<Option<SystemTime>> = self
            .watched
            .iter()
            .map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
            .collect();
        if self.stamps.as_ref() == Some(&now) {
            return false;
        }
        self.stamps = Some(now);
        let mut cfg = match std::fs::read_to_string(&self.watched[0]) {
            Ok(text) => parse(&text),
            Err(_) => Config::default(),
        };
        if let Some(dir) = &self.dir {
            load_extra_files_from(&mut cfg, dir);
        }
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

    /// Empty directory for a test's files, inside the cargo target dir (not /tmp).
    fn test_dir(name: &str) -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let dir = exe.parent().unwrap().join("rustype-test-tmp").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn app_mode_keys_match_regardless_of_case() {
        let c = parse("app_mode.Kitty = direct\n[app_modes]\n\"Google-Chrome\" = preedit\n");
        assert_eq!(c.mode_for_app(Some("google-chrome")), Mode::Preedit);
        assert_eq!(c.mode_for_app(Some("Google-Chrome")), Mode::Preedit);
        assert_eq!(c.mode_for_app(Some("google-chrome.desktop")), Mode::Preedit);
        assert_eq!(c.mode_for_app(Some("kitty")), Mode::Direct);
    }

    #[test]
    fn hash_inside_quotes_is_not_a_comment() {
        let c = parse("[macros]\nvn = \"Việt #1\" # trailing comment\n");
        assert_eq!(
            c.telex.macros.get("vn").map(|s| s.as_str()),
            Some("Việt #1")
        );
        let c = parse("mode = preedit # chú thích\n");
        assert_eq!(c.mode, Mode::Preedit);
    }

    #[test]
    fn watcher_picks_up_macro_and_words_edits() {
        let dir = test_dir("watcher");
        let cfg = dir.join("config");
        std::fs::write(&cfg, "mode = auto\n").unwrap();
        let mut w = Watcher::at(Some(cfg.clone()));
        assert!(w.current.telex.macros.is_empty());
        assert!(!w.refresh());

        // Creating the macro file is a change.
        let macro_file = dir.join("macro");
        std::fs::write(&macro_file, "vn: Việt Nam\n").unwrap();
        assert!(w.refresh());
        assert_eq!(
            w.current.telex.macros.get("vn").map(|s| s.as_str()),
            Some("Việt Nam")
        );

        // Editing it in place (mtime moved forward) is a change too.
        std::fs::write(&macro_file, "vn: Việt Nam Mới\n").unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(60);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&macro_file)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(w.refresh());
        assert_eq!(
            w.current.telex.macros.get("vn").map(|s| s.as_str()),
            Some("Việt Nam Mới")
        );
        assert!(!w.refresh());

        // The words file works the same way.
        std::fs::write(dir.join("words"), "github\n").unwrap();
        assert!(w.refresh());
        assert!(w.current.telex.custom_words.iter().any(|x| x == "github"));
    }

    #[test]
    fn input_method_line_matches_key_exactly() {
        assert!(is_input_method_line("input_method = telex"));
        assert!(is_input_method_line("  input_method=vni"));
        assert!(is_input_method_line("input_method\t= vni"));
        assert!(!is_input_method_line("input_method_extra = x"));
        assert!(!is_input_method_line("# input_method = telex"));
    }

    #[test]
    fn update_input_method_rewrites_only_the_key() {
        let dir = test_dir("input-method");
        let cfg = dir.join("config");
        std::fs::write(
            &cfg,
            "input_method_extra = keep\nmode = auto\ninput_method = telex\nvn = \"x\"\n",
        )
        .unwrap();
        update_input_method_at(&cfg, rustype_core::telex::InputMethod::Vni);
        let text = std::fs::read_to_string(&cfg).unwrap();
        assert_eq!(
            text,
            "input_method_extra = keep\nmode = auto\ninput_method = vni\nvn = \"x\"\n"
        );
        assert!(!dir.join(".config.tmp").exists());

        // Without the key, it is appended.
        std::fs::write(&cfg, "mode = auto\n").unwrap();
        update_input_method_at(&cfg, rustype_core::telex::InputMethod::Telex);
        let text = std::fs::read_to_string(&cfg).unwrap();
        assert_eq!(text, "mode = auto\ninput_method = telex\n");
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

//! Configuration management for Rustype on Windows.

use rustype_core::telex::InputMethod;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchShortcut {
    CtrlShift,
    AltZ,
}

#[derive(Clone, Debug)]
pub struct WinConfig {
    pub enabled: bool,
    pub input_method: InputMethod,
    pub modern_orthography: bool,
    pub auto_restore_english: bool,
    pub switch_shortcut: SwitchShortcut,
}

impl Default for WinConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            input_method: InputMethod::Telex,
            modern_orthography: false,
            auto_restore_english: true,
            switch_shortcut: SwitchShortcut::CtrlShift,
        }
    }
}

impl WinConfig {
    pub fn config_path() -> PathBuf {
        if let Ok(app_data) = std::env::var("APPDATA") {
            PathBuf::from(app_data).join("Rustype").join("config.ini")
        } else {
            PathBuf::from("config.ini")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut cfg = Self::default();
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once('=') {
                    let key = k.trim().to_lowercase();
                    let val = v.trim().to_lowercase();
                    match key.as_str() {
                        "enabled" => cfg.enabled = val == "true" || val == "1",
                        "input_method" => {
                            cfg.input_method = if val == "vni" {
                                InputMethod::Vni
                            } else {
                                InputMethod::Telex
                            };
                        }
                        "modern_orthography" => {
                            cfg.modern_orthography = val == "true" || val == "1";
                        }
                        "auto_restore_english" => {
                            cfg.auto_restore_english = val == "true" || val == "1";
                        }
                        "shortcut" => {
                            cfg.switch_shortcut = if val == "altz" {
                                SwitchShortcut::AltZ
                            } else {
                                SwitchShortcut::CtrlShift
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
        cfg
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = format!(
            "[General]\n\
             enabled = {}\n\
             input_method = {}\n\
             modern_orthography = {}\n\
             auto_restore_english = {}\n\
             shortcut = {}\n",
            self.enabled,
            match self.input_method {
                InputMethod::Telex => "telex",
                InputMethod::Vni => "vni",
            },
            self.modern_orthography,
            self.auto_restore_english,
            match self.switch_shortcut {
                SwitchShortcut::CtrlShift => "ctrlshift",
                SwitchShortcut::AltZ => "altz",
            }
        );
        std::fs::write(path, content)
    }
}

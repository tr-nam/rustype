//! `rustype-linux --doctor`: checks this GNOME session's setup for rustype-linux, in Vietnamese.

use crate::{addrfile, config, sys};
use rustype_core::telex::InputMethod;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const UNIT: &str = "org.freedesktop.IBus.session.GNOME.service";
const DROPIN_DIR: &str = "org.freedesktop.IBus.session.GNOME.service.d";
const PACKAGED_DROPIN: &str =
    "/usr/lib/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/rustype.conf";
const EXTENSION: &str = "rustype-focus@namtr.dev";
const SYSTEM_EXTENSION_DIR: &str = "/usr/share/gnome-shell/extensions";
const INPUT_SOURCE: &str = "('ibus', 'rustype')";

#[derive(Clone, Copy)]
enum Level {
    Ok,
    Warn,
    Fail,
}

#[derive(Default)]
struct Report {
    ok: u32,
    warn: u32,
    fail: u32,
}

impl Report {
    fn check(&mut self, level: Level, text: impl Display) {
        let mark = match level {
            Level::Ok => {
                self.ok += 1;
                "✓"
            }
            Level::Warn => {
                self.warn += 1;
                "!"
            }
            Level::Fail => {
                self.fail += 1;
                "✗"
            }
        };
        println!("[{mark}] {text}");
    }

    fn hint(&self, text: impl Display) {
        println!("    Gợi ý: {text}");
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// `$HOME/<rel>`, when HOME is set.
fn home_join(rel: &str) -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(rel))
}

pub fn run() {
    println!("=== Chẩn đoán Rustype (rustype-linux v{}) ===", VERSION);
    let mut r = Report::default();
    session_check(&mut r);
    dropin_check(&mut r);
    unit_check(&mut r);
    address_check(&mut r);
    input_source_check(&mut r);
    extension_check(&mut r);
    config_check(&mut r);
    summary(&r);
}

fn session_check(r: &mut Report) {
    let session = env_or("XDG_SESSION_TYPE", "chưa rõ");
    let desktop = env_or("XDG_CURRENT_DESKTOP", "chưa rõ");
    if desktop.to_lowercase().contains("gnome") {
        r.check(Level::Ok, format!("Phiên GNOME: {desktop} ({session})"));
    } else {
        r.check(
            Level::Warn,
            format!("Không phải phiên GNOME: {desktop} ({session})"),
        );
        r.hint(
            "rustype-linux được thiết kế cho GNOME; trên môi trường khác nó chưa được kiểm thử.",
        );
    }
}

/// The systemd drop-in that makes GNOME's IBus session start rustype-linux: the user's copy
/// wins over the packaged one.
fn dropin_check(r: &mut Report) {
    let user = home_join(&format!(".config/systemd/user/{DROPIN_DIR}/override.conf"));
    if let Some(p) = user.as_ref().filter(|p| p.exists()) {
        r.check(Level::Ok, format!("Drop-in người dùng: {}", p.display()));
    } else if Path::new(PACKAGED_DROPIN).exists() {
        r.check(Level::Ok, format!("Drop-in của gói: {PACKAGED_DROPIN}"));
    } else {
        let user_text = user.map(|p| p.display().to_string()).unwrap_or_default();
        r.check(
            Level::Fail,
            format!("Chưa có drop-in cho {UNIT}: {user_text} hoặc {PACKAGED_DROPIN}"),
        );
        r.hint("Cài đặt lại Rustype để có drop-in, rồi đăng xuất và đăng nhập lại.");
    }
}

/// Whether the GNOME IBus unit runs rustype-linux: its MainPID must be a process named so.
fn unit_check(r: &mut Report) {
    let out = Command::new("systemctl")
        .args([
            "--user",
            "show",
            "-p",
            "MainPID,ActiveState",
            "--value",
            UNIT,
        ])
        .output();
    let Ok(out) = out else {
        r.check(Level::Warn, "Không chạy được systemctl --user");
        return;
    };
    // With --value the two properties come one per line, in systemd's order: the PID is the
    // number, the state the word.
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let pid = text
        .lines()
        .find_map(|l| l.trim().parse::<i32>().ok())
        .filter(|p| *p > 0);
    let state = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && l.parse::<i32>().is_err())
        .unwrap_or("không rõ")
        .to_string();
    if state != "active" {
        r.check(
            Level::Fail,
            format!("Unit {UNIT} không chạy (ActiveState={state})"),
        );
        r.hint("Đăng xuất và đăng nhập lại để GNOME khởi động lại bộ gõ.");
        return;
    }
    let Some(pid) = pid else {
        r.check(Level::Fail, format!("Unit {UNIT} không có MainPID"));
        return;
    };
    match std::fs::read_link(format!("/proc/{pid}/exe")) {
        Ok(exe) => {
            let name = exe
                .file_name()
                .map(|n| {
                    n.to_string_lossy()
                        .trim_end_matches(" (deleted)")
                        .to_string()
                })
                .unwrap_or_default();
            if name == "rustype-linux" {
                r.check(
                    Level::Ok,
                    format!("Unit {UNIT} đang chạy rustype-linux (PID {pid})"),
                );
            } else {
                r.check(
                    Level::Fail,
                    format!("Unit {UNIT} đang chạy {name} (PID {pid}), không phải rustype-linux"),
                );
                r.hint("Kiểm tra drop-in ở mục trên, rồi đăng xuất và đăng nhập lại.");
            }
        }
        Err(e) => r.check(Level::Warn, format!("Không đọc được /proc/{pid}/exe: {e}")),
    }
}

/// The address file names the bus this session's clients use; its PID must still be alive.
fn address_check(r: &mut Report) {
    let path = addrfile::resolve_path(
        std::env::var_os("IBUS_ADDRESS_FILE"),
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
        &addrfile::machine_id(),
        &addrfile::display_name(
            std::env::var_os("WAYLAND_DISPLAY"),
            std::env::var_os("DISPLAY"),
        ),
    );
    let Ok(text) = std::fs::read_to_string(&path) else {
        r.check(
            Level::Warn,
            format!("Chưa có file địa chỉ {}", path.display()),
        );
        r.hint("rustype-linux chưa ghi file này, nghĩa là nó chưa chạy trong phiên này.");
        return;
    };
    match addrfile::parse_pid(&text) {
        Some(pid) if sys::pid_alive(pid) => r.check(
            Level::Ok,
            format!(
                "File địa chỉ {} trỏ tới tiến trình đang chạy (PID {pid})",
                path.display()
            ),
        ),
        Some(pid) => r.check(
            Level::Warn,
            format!("File địa chỉ {} trỏ tới PID {pid} đã thoát", path.display()),
        ),
        None => r.check(
            Level::Warn,
            format!("File địa chỉ {} không có IBUS_DAEMON_PID", path.display()),
        ),
    }
}

fn input_source_check(r: &mut Report) {
    let out = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.input-sources", "sources"])
        .output();
    match out {
        Ok(o) if String::from_utf8_lossy(&o.stdout).contains(INPUT_SOURCE) => {
            r.check(Level::Ok, format!("Nguồn nhập GNOME: đã có {INPUT_SOURCE}"))
        }
        Ok(_) => {
            r.check(
                Level::Warn,
                format!("Nguồn nhập GNOME: chưa có {INPUT_SOURCE}"),
            );
            r.hint("Vào Cài đặt -> Bàn phím -> Nguồn nhập, thêm 'Rustype (Tiếng Việt)'.");
        }
        Err(_) => r.check(Level::Warn, "Không chạy được gsettings để đọc nguồn nhập"),
    }
}

/// Whether the focus extension is installed: from a source install (under HOME) or from the
/// package (under /usr/share).
fn extension_installed(user_dir: Option<&Path>, system_dir: &Path) -> bool {
    user_dir.is_some_and(Path::exists) || system_dir.exists()
}

fn extension_check(r: &mut Report) {
    let user = home_join(&format!(".local/share/gnome-shell/extensions/{EXTENSION}"));
    let system = Path::new(SYSTEM_EXTENSION_DIR).join(EXTENSION);
    if extension_installed(user.as_deref(), &system) {
        r.check(
            Level::Ok,
            format!("GNOME Shell extension: đã cài đặt ({EXTENSION})"),
        );
    } else if env_or("XDG_SESSION_TYPE", "") == "wayland"
        && env_or("XDG_CURRENT_DESKTOP", "")
            .to_lowercase()
            .contains("gnome")
    {
        r.check(
            Level::Warn,
            format!("GNOME Shell extension: chưa cài đặt ({EXTENSION})"),
        );
        r.hint("Extension giúp nhận diện ứng dụng trên GNOME Wayland.");
    }
}

fn config_check(r: &mut Report) {
    let Some(p) = config::path() else {
        return;
    };
    if !p.exists() {
        r.check(
            Level::Warn,
            format!("Cấu hình: chưa có file {}", p.display()),
        );
        println!("    (Rustype đang dùng cấu hình mặc định: Telex, chế độ Auto).");
        r.hint("Chạy 'rustype-linux --write-config' để tạo file cấu hình mẫu.");
        return;
    }
    let mut cfg = config::parse(&std::fs::read_to_string(&p).unwrap_or_default());
    config::load_extra_files(&mut cfg);
    let mode = match cfg.mode {
        config::Mode::Auto => "auto",
        config::Mode::Preedit => "preedit",
        config::Mode::Direct => "direct",
    };
    let method = match cfg.telex.input_method {
        InputMethod::Telex => "Telex",
        InputMethod::Vni => "VNI",
    };
    r.check(Level::Ok, format!("Cấu hình: {}", p.display()));
    println!(
        "    Kiểu gõ: {method}, Chế độ: {mode}, Nhớ theo app: {}",
        cfg.remember_state_per_app
    );
    if cfg.telex.enable_macro {
        println!("    Gõ tắt (Macro): Bật ({} từ)", cfg.telex.macros.len());
    }
    if !cfg.telex.custom_words.is_empty() {
        println!("    Từ điển riêng: {} từ", cfg.telex.custom_words.len());
    }
}

fn summary(r: &Report) {
    println!(
        "\n--- Kết quả: {} tốt, {} cảnh báo, {} lỗi ---",
        r.ok, r.warn, r.fail
    );
    if r.fail == 0 && r.warn == 0 {
        println!("Mọi thứ hoàn hảo! Rustype đã sẵn sàng gõ tiếng Việt.");
    } else if r.fail > 0 {
        println!("Cần khắc phục các lỗi [✗] ở trên để Rustype hoạt động bình thường.");
    } else {
        println!(
            "Rustype có thể hoạt động được. Xem các cảnh báo [!] để có trải nghiệm tối ưu nhất."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_found_under_home_or_the_package_path() {
        let base = std::env::temp_dir().join(format!("rustype-doctor-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let user = base.join("user").join(EXTENSION);
        let system = base.join("system").join(EXTENSION);
        assert!(!extension_installed(Some(&user), &system));
        assert!(!extension_installed(None, &system));
        std::fs::create_dir_all(&system).unwrap();
        assert!(extension_installed(Some(&user), &system));
        assert!(extension_installed(None, &system));
        let _ = std::fs::remove_dir_all(&base);
    }
}

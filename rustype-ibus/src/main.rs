//! Rustype: a tiny Vietnamese Telex input method engine for IBus.

use rustype_core::telex;
use rustype_ibus::{config, ibus};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const USAGE: &str = "\
ibus-engine-rustype: bộ gõ tiếng Việt Telex cho IBus

  --ibus            chạy engine (IBus gọi lệnh này)
  --xml [exec]      in file component cho /usr/share/ibus/component
                    (exec: lệnh ghi vào <exec>, mặc định là chính file này)
  --write-config    tạo ~/.config/rustype/config nếu chưa có
  --doctor          kiểm tra chẩn đoán hệ thống và phát hiện lỗi
  --feed            đọc từng dòng phím Telex từ stdin, in chữ ra (để kiểm thử)
  --version";

/// Whether a process with this pid exists.
fn pid_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

/// The address in one IBus bus file, but only when the daemon that wrote it is still running.
/// A leftover file from a dead daemon, or from another bus manager, yields None.
fn live_bus_address(file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .map(|v| v.trim())
    };
    let addr = field("IBUS_ADDRESS=").filter(|a| !a.is_empty())?;
    let pid: u32 = field("IBUS_DAEMON_PID=")?.parse().ok()?;
    pid_alive(pid).then(|| addr.to_string())
}

/// The address from the most recently modified file in `bus_dir` whose daemon is alive.
fn newest_live_bus_address(bus_dir: &Path) -> Option<String> {
    let mut best: Option<(SystemTime, String)> = None;
    for entry in std::fs::read_dir(bus_dir).ok()?.flatten() {
        let Some(addr) = live_bus_address(&entry.path()) else {
            continue;
        };
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(t, _)| mtime > *t) {
            best = Some((mtime, addr));
        }
    }
    best.map(|(_, addr)| addr)
}

/// Where the IBus daemon of this session listens, the same way libibus finds it.
/// Addresses read from bus files are accepted only when their daemon is alive.
fn ibus_address() -> Option<String> {
    if let Ok(a) = std::env::var("IBUS_ADDRESS") {
        if !a.is_empty() {
            return Some(a);
        }
    }
    if let Some(f) = std::env::var_os("IBUS_ADDRESS_FILE") {
        if let Some(addr) = live_bus_address(&PathBuf::from(f)) {
            return Some(addr);
        }
    }
    let bus_dir = config::path()?.parent()?.parent()?.join("ibus").join("bus");
    let machine_id = ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())?
        .trim()
        .to_string();
    let (host, display) = match std::env::var("WAYLAND_DISPLAY") {
        Ok(w) if !w.is_empty() => ("unix".to_string(), w),
        _ => {
            let d = std::env::var("DISPLAY").unwrap_or_default();
            let (h, rest) = d.split_once(':').unwrap_or(("", "0"));
            let num = rest.split('.').next().unwrap_or("0").to_string();
            (
                if h.is_empty() {
                    "unix".to_string()
                } else {
                    h.to_string()
                },
                num,
            )
        }
    };
    let candidate = bus_dir.join(format!("{machine_id}-{host}-{display}"));
    if let Some(addr) = live_bus_address(&candidate) {
        return Some(addr);
    }
    newest_live_bus_address(&bus_dir)
}

/// Escapes text for use inside XML element content or attribute values.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn default_exe() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "ibus-engine-rustype".into())
}

fn component_xml(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<component>
    <name>{name}</name>
    <description>Rustype - bộ gõ tiếng Việt Telex</description>
    <exec>{exe} --ibus</exec>
    <version>{ver}</version>
    <author>NamTr_n</author>
    <license>MIT</license>
    <homepage>https://namtr.dev</homepage>
    <textdomain>rustype</textdomain>
    <engines>
        <engine>
            <name>{engine}</name>
            <language>vi</language>
            <license>MIT</license>
            <author>NamTr_n</author>
            <icon>rustype</icon>
            <layout>us</layout>
            <longname>Rustype</longname>
            <description>Bộ gõ siêu nhẹ chạy bằng Rust</description>
            <rank>99</rank>
            <symbol>R</symbol>
        </engine>
    </engines>
</component>
"#,
        name = ibus::COMPONENT_NAME,
        exe = xml_escape(exe),
        engine = ibus::ENGINE_NAME,
        ver = env!("CARGO_PKG_VERSION"),
    )
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    match arg.as_str() {
        "--ibus" => {
            let Some(addr) = ibus_address() else {
                eprintln!("rustype: không tìm thấy địa chỉ IBus");
                std::process::exit(1);
            };
            match ibus::run(&addr) {
                Ok(()) => {}
                // The daemon went away (logout, `ibus restart`): exit quietly.
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {}
                Err(e) => {
                    eprintln!("rustype: {e}");
                    std::process::exit(1);
                }
            }
        }
        "--xml" => {
            let exe = std::env::args().nth(2).unwrap_or_else(default_exe);
            print!("{}", component_xml(&exe));
        }
        "--write-config" => {
            let Some(p) = config::path() else {
                std::process::exit(1)
            };
            if p.exists() {
                println!("{} đã có, giữ nguyên", p.display());
            } else {
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(&p, config::DEFAULT_FILE) {
                    eprintln!("rustype: {e}");
                    std::process::exit(1);
                }
                println!("đã tạo {}", p.display());
            }
        }
        "--doctor" => doctor(),
        "--feed" => {
            let mut t = telex::Telex::new(telex::Config::default());
            let stdin = io::stdin();
            let mut out = io::BufWriter::new(io::stdout().lock());
            for line in stdin.lock().lines() {
                let line = line.unwrap_or_default();
                let _ = writeln!(out, "{}", t.feed(&line));
            }
        }
        "--version" => println!("rustype {}", env!("CARGO_PKG_VERSION")),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn doctor() {
    println!(
        "=== Chẩn đoán hệ thống Rustype v{} ===",
        env!("CARGO_PKG_VERSION")
    );
    let mut ok_count = 0;
    let mut warn_count = 0;
    let mut fail_count = 0;

    // 1. Desktop & Session
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "chưa rõ".into());
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "chưa rõ".into());
    println!("[i] Môi trường: {} ({})", desktop, session);

    // 2. IBus daemon & address
    match ibus_address() {
        Some(addr) => {
            println!("[✓] IBus daemon: đang chạy");
            println!("    Địa chỉ: {}", addr);
            ok_count += 1;
        }
        None => {
            println!("[✗] IBus daemon: không tìm thấy socket hoặc daemon chưa chạy");
            println!("    Gợi ý: Thử chạy lệnh 'ibus restart' hoặc kiểm tra biến môi trường IBUS_ADDRESS.");
            fail_count += 1;
        }
    }

    // 3. IBus Component XML
    let xml_path = std::path::Path::new("/usr/share/ibus/component/rustype.xml");
    if xml_path.exists() {
        println!("[✓] File đăng ký: /usr/share/ibus/component/rustype.xml tồn tại");
        ok_count += 1;
    } else {
        println!("[✗] File đăng ký: /usr/share/ibus/component/rustype.xml CHƯA CÓ");
        println!("    Gợi ý: Chạy './install.sh' để đăng ký Rustype với IBus (cần quyền sudo).");
        fail_count += 1;
    }

    // 4. GNOME input-sources
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.input-sources", "sources"])
        .output()
    {
        let s = String::from_utf8_lossy(&output.stdout);
        if s.contains("('ibus', 'rustype')") {
            println!("[✓] Nguồn nhập GNOME: Rustype đã được bật trong danh sách");
            ok_count += 1;
        } else {
            println!("[!] Nguồn nhập GNOME: chưa có ('ibus', 'rustype')");
            println!("    Gợi ý: Vào Cài đặt -> Bàn phím -> Thêm nguồn nhập 'Rustype (Tiếng Việt)' hoặc chạy './install.sh'.");
            warn_count += 1;
        }
    }

    // 5. GNOME Shell Extension
    if let Some(home) = std::env::var_os("HOME") {
        let ext_dir =
            PathBuf::from(home).join(".local/share/gnome-shell/extensions/rustype-focus@namtr.dev");
        if ext_dir.exists() {
            println!("[✓] GNOME Shell extension: đã cài đặt (rustype-focus@namtr.dev)");
            ok_count += 1;
        } else if session == "wayland" && desktop.to_lowercase().contains("gnome") {
            println!("[!] GNOME Shell extension: chưa cài đặt (khuyên dùng trên GNOME Wayland để nhận diện app)");
            println!("    Gợi ý: Chạy './install.sh' để tự động cài đặt extension.");
            warn_count += 1;
        }
    }

    // 6. Cấu hình
    if let Some(p) = config::path() {
        if p.exists() {
            let mut cfg = config::parse(&std::fs::read_to_string(&p).unwrap_or_default());
            config::load_extra_files(&mut cfg);
            let mode_str = match cfg.mode {
                config::Mode::Auto => "auto",
                config::Mode::Preedit => "preedit",
                config::Mode::Direct => "direct",
            };
            let method_str = match cfg.telex.input_method {
                telex::InputMethod::Telex => "Telex",
                telex::InputMethod::Vni => "VNI",
            };
            println!("[✓] Cấu hình: {}", p.display());
            println!(
                "    Kiểu gõ: {}, Chế độ: {}, Nhớ theo app: {}",
                method_str, mode_str, cfg.remember_state_per_app
            );
            if cfg.telex.enable_macro {
                println!("    Gõ tắt (Macro): Bật ({} từ)", cfg.telex.macros.len());
            }
            if !cfg.telex.custom_words.is_empty() {
                println!("    Từ điển riêng: {} từ", cfg.telex.custom_words.len());
            }
            ok_count += 1;
        } else {
            println!("[!] Cấu hình: chưa có file {}", p.display());
            println!("    (Rustype đang sử dụng cấu hình mặc định: Telex, chế độ Auto).");
            println!(
                "    Gợi ý: Chạy 'ibus-engine-rustype --write-config' để tạo file cấu hình mẫu."
            );
            warn_count += 1;
        }
    }

    // Kết luận
    println!(
        "\n--- Kết quả: {} tốt, {} cảnh báo, {} lỗi ---",
        ok_count, warn_count, fail_count
    );
    if fail_count == 0 && warn_count == 0 {
        println!("Mọi thứ hoàn hảo! Rustype đã sẵn sàng gõ tiếng Việt.");
    } else if fail_count > 0 {
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

    /// Empty directory for a test's files, inside the cargo target dir (not /tmp).
    fn test_dir(name: &str) -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let dir = exe.parent().unwrap().join("rustype-test-tmp").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes a bus file with the given address and daemon pid, and sets its mtime.
    fn write_bus_file(dir: &Path, name: &str, addr: &str, pid: Option<u32>, mtime: SystemTime) {
        let file = dir.join(name);
        let mut text = format!("IBUS_ADDRESS={addr}\n");
        if let Some(pid) = pid {
            text.push_str(&format!("IBUS_DAEMON_PID={pid}\n"));
        }
        std::fs::write(&file, text).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
    }

    const DEAD_PID: u32 = 999_999_999;

    #[test]
    fn bus_file_selection_skips_dead_and_pidless_files() {
        let dir = test_dir("bus-select");
        let now = SystemTime::now();
        let me = std::process::id();
        // Newest file belongs to a dead daemon (like a stale fcitx5 leftover); must be skipped.
        write_bus_file(&dir, "stale", "unix:path=/stale", Some(DEAD_PID), now);
        // Newest live-looking file without a pid cannot be verified; must be skipped.
        write_bus_file(
            &dir,
            "nopid",
            "unix:path=/nopid",
            None,
            now + std::time::Duration::from_secs(5),
        );
        write_bus_file(
            &dir,
            "old-live",
            "unix:path=/old",
            Some(me),
            now - std::time::Duration::from_secs(600),
        );
        write_bus_file(
            &dir,
            "new-live",
            "unix:path=/new",
            Some(me),
            now - std::time::Duration::from_secs(60),
        );
        assert_eq!(
            newest_live_bus_address(&dir).as_deref(),
            Some("unix:path=/new")
        );
    }

    #[test]
    fn bus_file_selection_returns_none_without_live_daemon() {
        let dir = test_dir("bus-none");
        write_bus_file(
            &dir,
            "stale",
            "unix:path=/stale",
            Some(DEAD_PID),
            SystemTime::now(),
        );
        assert_eq!(newest_live_bus_address(&dir), None);
        assert_eq!(live_bus_address(&dir.join("missing")), None);
        assert_eq!(newest_live_bus_address(&dir.join("no-such-dir")), None);
    }

    #[test]
    fn live_bus_address_checks_the_pid() {
        let dir = test_dir("bus-one");
        write_bus_file(
            &dir,
            "me",
            "unix:abstract=me",
            Some(std::process::id()),
            SystemTime::now(),
        );
        write_bus_file(
            &dir,
            "dead",
            "unix:abstract=dead",
            Some(DEAD_PID),
            SystemTime::now(),
        );
        assert_eq!(
            live_bus_address(&dir.join("me")).as_deref(),
            Some("unix:abstract=me")
        );
        assert_eq!(live_bus_address(&dir.join("dead")), None);
    }

    #[test]
    fn xml_escape_handles_markup_characters() {
        assert_eq!(
            xml_escape("/opt/a&b/<x>\"'"),
            "/opt/a&amp;b/&lt;x&gt;&quot;&apos;"
        );
        let xml = component_xml("/opt/a&b/rustype");
        assert!(xml.contains("<exec>/opt/a&amp;b/rustype --ibus</exec>"));
    }
}

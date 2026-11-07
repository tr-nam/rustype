//! Rustype: a tiny Vietnamese Telex input method engine for IBus.

mod config;
mod dbus;
mod ibus;
mod ime;

use rustype_core::telex;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

const USAGE: &str = "\
ibus-engine-rustype: bộ gõ tiếng Việt Telex cho IBus

  --ibus            chạy engine (IBus gọi lệnh này)
  --xml             in file component cho /usr/share/ibus/component
  --write-config    tạo ~/.config/rustype/config nếu chưa có
  --doctor          kiểm tra chẩn đoán hệ thống và phát hiện lỗi
  --feed            đọc từng dòng phím Telex từ stdin, in chữ ra (để kiểm thử)
  --version";

/// Where the IBus daemon of this session listens, the same way libibus finds it.
fn ibus_address() -> Option<String> {
    if let Ok(a) = std::env::var("IBUS_ADDRESS") {
        if !a.is_empty() {
            return Some(a);
        }
    }
    let parse_file = |p: &std::path::Path| -> Option<String> {
        let text = std::fs::read_to_string(p).ok()?;
        text.lines().find_map(|l| {
            let val = l.strip_prefix("IBUS_ADDRESS=")?.trim();
            if val.is_empty() {
                None
            } else {
                Some(val.to_string())
            }
        })
    };
    if let Some(f) = std::env::var_os("IBUS_ADDRESS_FILE") {
        if let Some(addr) = parse_file(&PathBuf::from(f)) {
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
    if let Some(addr) = parse_file(&candidate) {
        return Some(addr);
    }
    // Fallback: search any file in bus_dir
    if let Ok(entries) = std::fs::read_dir(&bus_dir) {
        for entry in entries.flatten() {
            if let Some(addr) = parse_file(&entry.path()) {
                return Some(addr);
            }
        }
    }
    None
}

fn component_xml() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "ibus-engine-rustype".into());
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
        "--xml" => print!("{}", component_xml()),
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

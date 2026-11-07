//! Rustype: a tiny Vietnamese Telex input method engine for IBus.

mod config;
mod dbus;
mod ibus;
mod ime;
mod tables;
mod telex;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

const USAGE: &str = "\
ibus-engine-rustype: bộ gõ tiếng Việt Telex cho IBus

  --ibus            chạy engine (IBus gọi lệnh này)
  --xml             in file component cho /usr/share/ibus/component
  --write-config    tạo ~/.config/rustype/config nếu chưa có
  --feed            đọc từng dòng phím Telex từ stdin, in chữ ra (để kiểm thử)
  --version";

/// Where the IBus daemon of this session listens, the same way libibus finds it.
fn ibus_address() -> Option<String> {
    if let Ok(a) = std::env::var("IBUS_ADDRESS") {
        if !a.is_empty() {
            return Some(a);
        }
    }
    let file = match std::env::var_os("IBUS_ADDRESS_FILE") {
        Some(f) => PathBuf::from(f),
        None => {
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
                    (if h.is_empty() { "unix".to_string() } else { h.to_string() }, num)
                }
            };
            config::path()?.parent()?.parent()?.join("ibus").join("bus").join(format!("{machine_id}-{host}-{display}"))
        }
    };
    let text = std::fs::read_to_string(file).ok()?;
    text.lines().find_map(|l| l.strip_prefix("IBUS_ADDRESS=")).map(|s| s.trim().to_string())
}

fn component_xml() -> String {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "ibus-engine-rustype".into());
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
            <icon>input-keyboard</icon>
            <layout>us</layout>
            <longname>Rustype</longname>
            <description>Bộ gõ siêu nhẹ chạy bằng Rust</description>
            <rank>99</rank>
            <symbol>vi</symbol>
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
            let Some(p) = config::path() else { std::process::exit(1) };
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

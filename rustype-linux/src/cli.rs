//! Command line of `rustype-linux`: the daemon by default, and the tools around it.

use crate::dbus::{Connection, METHOD_CALL};
use crate::{addrfile, config, doctor, server, service, sys};
use rustype_core::telex;
use std::io::{self, BufRead, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "\
usage: rustype-linux [--address unix:path=PATH] [--replace] [--own-session-name]
       rustype-linux --write-config | --doctor | --feed | --version";

#[derive(Debug, Default)]
struct Opts {
    address: Option<String>,
    replace: bool,
    own_session_name: bool,
}

#[derive(Debug)]
enum Command {
    Run(Opts),
    WriteConfig,
    Doctor,
    Feed,
    Version,
    Help,
}

fn parse_args<I: Iterator<Item = String>>(mut args: I) -> Result<Command, String> {
    let mut opts = Opts::default();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--version" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--write-config" => return Ok(Command::WriteConfig),
            "--doctor" => return Ok(Command::Doctor),
            "--feed" => return Ok(Command::Feed),
            "--replace" => opts.replace = true,
            "--own-session-name" => opts.own_session_name = true,
            "--address" => {
                opts.address = Some(args.next().ok_or("--address needs a value")?);
            }
            _ => {
                if let Some(v) = a.strip_prefix("--address=") {
                    opts.address = Some(v.to_string());
                } else {
                    return Err(format!("unknown argument: {a}"));
                }
            }
        }
    }
    Ok(Command::Run(opts))
}

/// The socket path from `--address unix:path=...` (abstract sockets are not supported yet).
fn address_socket(address: &str) -> io::Result<PathBuf> {
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidInput, m.to_string());
    let params = address
        .strip_prefix("unix:")
        .ok_or_else(|| bad("only unix:path= addresses are supported"))?;
    for kv in params.split(',') {
        if let Some(p) = kv.strip_prefix("path=") {
            return Ok(PathBuf::from(p));
        }
    }
    Err(bad("the unix address has no path="))
}

/// `$XDG_RUNTIME_DIR/rustype/bus`, the 0700 directory the spec asks for.
fn default_socket() -> io::Result<PathBuf> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "XDG_RUNTIME_DIR is not set; pass --address unix:path=...",
            )
        })?;
    Ok(base.join("rustype").join("bus"))
}

/// Creates the parent directory (0700 when we create it) and binds the listener.
/// A socket file that nothing listens on is stale and removed; a live one is refused.
fn bind_socket(path: &Path, replace: bool) -> io::Result<UnixListener> {
    if let Some(dir) = path.parent() {
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        use std::os::unix::fs::FileTypeExt;
        if !meta.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists and is not a socket", path.display()),
            ));
        }
        if UnixStream::connect(path).is_ok() && !replace {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!(
                    "{} is served by a live bus; pass --replace to take it over",
                    path.display()
                ),
            ));
        }
        std::fs::remove_file(path)?;
    }
    UnixListener::bind(path)
}

/// Connects to the session bus and owns org.freedesktop.IBus there (the systemd unit's BusName).
fn own_session_name() -> io::Result<Option<Connection>> {
    let Some(addr) = std::env::var_os("DBUS_SESSION_BUS_ADDRESS") else {
        eprintln!(
            "rustype-linux: DBUS_SESSION_BUS_ADDRESS is not set; not claiming the session name"
        );
        return Ok(None);
    };
    let addr = addr.to_string_lossy().into_owned();
    let mut conn = Connection::open(&addr)?;
    let code = conn.request_name("org.freedesktop.IBus")?;
    if code != 1 {
        return Err(io::Error::other(format!(
            "cannot own org.freedesktop.IBus on the session bus (reply {code})"
        )));
    }
    Ok(Some(conn))
}

/// Keeps the session-bus connection drained, so its queue cannot grow: signals are discarded and
/// method calls get UnknownMethod. The name stays owned for as long as the process runs.
fn serve_session(mut conn: Connection) {
    std::thread::spawn(move || loop {
        let Ok(m) = conn.next_message() else {
            return;
        };
        if m.mtype == METHOD_CALL {
            let mut batch = Vec::new();
            conn.error(
                &mut batch,
                &m,
                "org.freedesktop.DBus.Error.UnknownMethod",
                "rustype-linux does not serve the session bus",
            );
            if conn.flush(&batch).is_err() {
                return;
            }
        }
    });
}

/// Removes what this process created, when it exits.
struct Cleanup {
    socket: PathBuf,
    address_file: PathBuf,
    pid: i32,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
        addrfile::remove_if_ours(&self.address_file, self.pid);
    }
}

fn run(opts: Opts) -> io::Result<()> {
    sys::install_stop_handlers();
    let pid = std::process::id() as i32;

    // Refuse before touching anything: the address file must not name a live other daemon.
    let address_file = addrfile::resolve_path(
        std::env::var_os("IBUS_ADDRESS_FILE"),
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
        &addrfile::machine_id(),
        &addrfile::display_name(
            std::env::var_os("WAYLAND_DISPLAY"),
            std::env::var_os("DISPLAY"),
        ),
    );
    addrfile::guard(&address_file, pid, opts.replace)?;

    let socket = match &opts.address {
        Some(a) => address_socket(a)?,
        None => default_socket()?,
    };
    let listener = bind_socket(&socket, opts.replace)?;
    let _cleanup = Cleanup {
        socket: socket.clone(),
        address_file: address_file.clone(),
        pid,
    };

    let guid = sys::random_hex(16);
    let address = format!("unix:path={},guid={guid}", socket.display());
    addrfile::write(&address_file, &address, pid)?;

    if opts.own_session_name {
        if let Some(conn) = own_session_name()? {
            serve_session(conn);
        }
    }

    eprintln!("rustype-linux {VERSION}: listening on {address}");
    let ibus = service::IBusService::new(address);
    let mut srv = server::Server::new(listener, guid, ibus)?;
    srv.run()
}

/// Creates the user config file with the sample text, unless it already exists.
fn write_config() {
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

/// Reads one Telex key sequence per stdin line and prints the composed text (used by golden tests).
fn feed() {
    let mut t = telex::Telex::new(telex::Config::default());
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line.unwrap_or_default();
        let _ = writeln!(out, "{}", t.feed(&line));
    }
}

/// The `rustype-linux` command line.
pub fn main() {
    match parse_args(std::env::args().skip(1)) {
        Ok(Command::Version) => println!("rustype-linux {VERSION}"),
        Ok(Command::Help) => println!("{USAGE}"),
        Ok(Command::WriteConfig) => write_config(),
        Ok(Command::Doctor) => doctor::run(),
        Ok(Command::Feed) => feed(),
        Ok(Command::Run(opts)) => {
            if let Err(e) = run(opts) {
                eprintln!("rustype-linux: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("rustype-linux: {e}\n{USAGE}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_and_run_flags_parse() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(matches!(
            parse_args(s(&["--version"]).into_iter()),
            Ok(Command::Version)
        ));
        match parse_args(s(&["--address", "unix:path=/x/y", "--replace"]).into_iter()) {
            Ok(Command::Run(o)) => {
                assert_eq!(o.address.as_deref(), Some("unix:path=/x/y"));
                assert!(o.replace);
                assert!(!o.own_session_name);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(parse_args(s(&["--bogus"]).into_iter()).is_err());
        assert_eq!(
            address_socket("unix:path=/run/a,guid=00").unwrap(),
            PathBuf::from("/run/a")
        );
    }

    #[test]
    fn tool_flags_parse() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(matches!(
            parse_args(s(&["--write-config"]).into_iter()),
            Ok(Command::WriteConfig)
        ));
        assert!(matches!(
            parse_args(s(&["--doctor"]).into_iter()),
            Ok(Command::Doctor)
        ));
        assert!(matches!(
            parse_args(s(&["--feed"]).into_iter()),
            Ok(Command::Feed)
        ));
        assert!(matches!(
            parse_args(std::iter::empty()),
            Ok(Command::Run(_))
        ));
    }
}

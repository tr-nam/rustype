//! The few libc calls std does not wrap: poll, SO_PEERCRED, kill, and stop-signal handling.
//! libc is already linked by std, so plain `extern "C"` declarations are enough.

use std::io;
use std::os::raw::{c_int, c_short, c_void};
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PollFd {
    pub fd: c_int,
    pub events: c_short,
    pub revents: c_short,
}

pub const POLLIN: c_short = 0x001;
pub const POLLOUT: c_short = 0x004;
pub const POLLERR: c_short = 0x008;
pub const POLLHUP: c_short = 0x010;

const SOL_SOCKET: c_int = 1;
const SO_PEERCRED: c_int = 17;
const SIGINT: c_int = 2;
const SIGTERM: c_int = 15;
const EPERM: i32 = 1;

#[repr(C)]
struct Ucred {
    pid: c_int,
    uid: u32,
    gid: u32,
}

extern "C" {
    fn poll(fds: *mut PollFd, nfds: usize, timeout: c_int) -> c_int;
    fn getsockopt(
        fd: c_int,
        level: c_int,
        optname: c_int,
        optval: *mut c_void,
        optlen: *mut u32,
    ) -> c_int;
    fn geteuid() -> u32;
    fn kill(pid: c_int, sig: c_int) -> c_int;
    fn signal(sig: c_int, handler: usize) -> usize;
}

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_stop_signal(_sig: c_int) {
    STOP.store(true, Ordering::SeqCst);
}

/// Installs SIGINT/SIGTERM handlers that only set a flag; the poll loop checks it.
pub fn install_stop_handlers() {
    // SAFETY: the handler is async-signal-safe (one atomic store).
    unsafe {
        signal(SIGINT, on_stop_signal as *const () as usize);
        signal(SIGTERM, on_stop_signal as *const () as usize);
    }
}

pub fn stop_requested() -> bool {
    STOP.load(Ordering::SeqCst)
}

/// Waits until one of `fds` is ready or the timeout passes. EINTR is not an error.
pub fn poll_fds(fds: &mut [PollFd], timeout_ms: i32) -> io::Result<()> {
    // SAFETY: the pointer and length describe the slice exactly.
    let r = unsafe { poll(fds.as_mut_ptr(), fds.len(), timeout_ms) };
    if r < 0 {
        let e = io::Error::last_os_error();
        if e.kind() == io::ErrorKind::Interrupted {
            return Ok(());
        }
        return Err(e);
    }
    Ok(())
}

pub fn euid() -> u32 {
    // SAFETY: geteuid has no preconditions.
    unsafe { geteuid() }
}

/// Pid and uid of the process on the other end of a unix socket.
pub fn peer_cred(fd: RawFd) -> io::Result<(i32, u32)> {
    let mut cred = Ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<Ucred>() as u32;
    // SAFETY: cred and len are valid for the size passed.
    let r = unsafe {
        getsockopt(
            fd,
            SOL_SOCKET,
            SO_PEERCRED,
            &mut cred as *mut Ucred as *mut c_void,
            &mut len,
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((cred.pid, cred.uid))
}

/// True when `pid` exists (kill with signal 0 succeeds, or fails with EPERM).
pub fn pid_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 only probes for existence.
    if unsafe { kill(pid, 0) } == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() == Some(EPERM)
}

/// `n` random bytes as lowercase hex, from /dev/urandom (time-based fallback).
pub fn random_hex(n: usize) -> String {
    use std::io::Read;
    let mut buf = vec![0u8; n];
    let ok = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_ok();
    if !ok {
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
            ^ ((std::process::id() as u64) << 32);
        for b in buf.iter_mut() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *b = seed as u8;
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_pid_is_alive_and_zero_is_not() {
        assert!(pid_alive(std::process::id() as i32));
        assert!(!pid_alive(0));
        assert!(!pid_alive(-1));
    }

    #[test]
    fn random_hex_has_expected_shape() {
        let h = random_hex(16);
        assert_eq!(h.len(), 32);
        assert!(h
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    }
}

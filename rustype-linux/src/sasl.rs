//! Server side of the D-Bus SASL handshake, EXTERNAL only (what GDBus and libibus send):
//! NUL, `AUTH EXTERNAL <hex uid>` -> `OK <guid>`, bare `AUTH` -> `REJECTED EXTERNAL`,
//! `NEGOTIATE_UNIX_FD` -> `ERROR`, `BEGIN` switches the connection to messages.

const MAX_LINE: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// Need more bytes.
    Wait,
    /// BEGIN was read; the bytes after its line are the first message.
    Begun,
    /// The client broke the protocol or tried BEGIN without authenticating.
    Failed,
}

pub struct Sasl {
    guid: String,
    euid: u32,
    peer_uid: u32,
    nul_seen: bool,
    authed: bool,
}

fn decode_hex(s: &str) -> Option<String> {
    if s.is_empty() || !s.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

impl Sasl {
    /// `guid` is the server GUID (32 hex digits). `euid` is ours, `peer_uid` comes from SO_PEERCRED.
    pub fn new(guid: &str, euid: u32, peer_uid: u32) -> Self {
        Sasl {
            guid: guid.to_string(),
            euid,
            peer_uid,
            nul_seen: false,
            authed: false,
        }
    }

    /// Consumes complete lines from `buf`. Returns the bytes to send and what happened.
    /// After `Event::Begun`, the unread remainder of `buf` is message data.
    pub fn feed(&mut self, buf: &mut Vec<u8>) -> (Vec<u8>, Event) {
        let mut out = Vec::new();
        if !self.nul_seen {
            match buf.first() {
                None => return (out, Event::Wait),
                Some(0) => {
                    buf.remove(0);
                    self.nul_seen = true;
                }
                Some(_) => return (out, Event::Failed),
            }
        }
        loop {
            let Some(pos) = buf.iter().position(|&b| b == b'\n') else {
                if buf.len() > MAX_LINE {
                    return (out, Event::Failed);
                }
                return (out, Event::Wait);
            };
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let text = String::from_utf8_lossy(&line);
            let mut words = text.split_whitespace();
            let cmd = words.next().unwrap_or("");
            match cmd {
                "AUTH" => {
                    let mech = words.next();
                    let arg = words.next();
                    match (mech, arg) {
                        (Some("EXTERNAL"), Some(hex)) if self.uid_ok(hex) => {
                            self.authed = true;
                            out.extend_from_slice(format!("OK {}\r\n", self.guid).as_bytes());
                        }
                        _ => out.extend_from_slice(b"REJECTED EXTERNAL\r\n"),
                    }
                }
                "NEGOTIATE_UNIX_FD" => out.extend_from_slice(b"ERROR\r\n"),
                "CANCEL" => out.extend_from_slice(b"REJECTED EXTERNAL\r\n"),
                "BEGIN" => {
                    return if self.authed {
                        (out, Event::Begun)
                    } else {
                        (out, Event::Failed)
                    };
                }
                _ => out.extend_from_slice(b"ERROR\r\n"),
            }
        }
    }

    /// The hex-encoded uid must be our euid, and the kernel must say the peer is that uid.
    fn uid_ok(&self, hex: &str) -> bool {
        self.peer_uid == self.euid
            && decode_hex(hex).is_some_and(|uid| uid == self.euid.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUID: &str = "0123456789abcdef0123456789abcdef";

    fn sasl() -> Sasl {
        Sasl::new(GUID, 1000, 1000)
    }

    #[test]
    fn full_transcript_then_begin() {
        let mut s = sasl();
        let mut buf = b"\0AUTH EXTERNAL 31303030\r\nBEGIN\r\n\x6c\x01\x00\x01".to_vec();
        let (out, ev) = s.feed(&mut buf);
        assert_eq!(out, format!("OK {GUID}\r\n").into_bytes());
        assert_eq!(ev, Event::Begun);
        // The first message's bytes were not consumed.
        assert_eq!(buf, b"\x6c\x01\x00\x01");
    }

    #[test]
    fn bare_auth_is_rejected_and_retry_works() {
        let mut s = sasl();
        let mut buf = b"\0AUTH\r\nAUTH EXTERNAL 31303030\r\nBEGIN\r\n".to_vec();
        let (out, ev) = s.feed(&mut buf);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("REJECTED EXTERNAL\r\nOK {GUID}\r\n")
        );
        assert_eq!(ev, Event::Begun);
    }

    #[test]
    fn negotiate_unix_fd_gets_error_and_handshake_continues() {
        let mut s = sasl();
        let mut buf = b"\0NEGOTIATE_UNIX_FD\r\nAUTH EXTERNAL 31303030\r\nBEGIN\r\n".to_vec();
        let (out, ev) = s.feed(&mut buf);
        assert!(out.starts_with(b"ERROR\r\n"));
        assert_eq!(ev, Event::Begun);
    }

    #[test]
    fn wrong_uid_is_rejected_and_begin_fails() {
        let mut s = sasl();
        // hex("0") = 30; euid is 1000
        let mut buf = b"\0AUTH EXTERNAL 30\r\nBEGIN\r\n".to_vec();
        let (out, ev) = s.feed(&mut buf);
        assert_eq!(out, b"REJECTED EXTERNAL\r\n");
        assert_eq!(ev, Event::Failed);
    }

    #[test]
    fn peer_uid_mismatch_is_rejected_even_when_hex_matches() {
        let mut s = Sasl::new(GUID, 1000, 1001);
        let mut buf = b"\0AUTH EXTERNAL 31303030\r\n".to_vec();
        let (out, ev) = s.feed(&mut buf);
        assert_eq!(out, b"REJECTED EXTERNAL\r\n");
        assert_eq!(ev, Event::Wait);
    }

    #[test]
    fn begin_without_auth_fails() {
        let mut s = sasl();
        let mut buf = b"\0BEGIN\r\n".to_vec();
        assert_eq!(s.feed(&mut buf).1, Event::Failed);
    }

    #[test]
    fn missing_nul_fails_and_partial_lines_wait() {
        let mut s = sasl();
        assert_eq!(s.feed(&mut b"AUTH".to_vec()).1, Event::Failed);

        let mut s = sasl();
        let mut buf = b"\0AUTH EXT".to_vec();
        let (out, ev) = s.feed(&mut buf);
        assert!(out.is_empty());
        assert_eq!(ev, Event::Wait);
        buf.extend_from_slice(b"ERNAL 31303030\r\nBEGIN\r\n");
        let (out, ev) = s.feed(&mut buf);
        assert_eq!(out, format!("OK {GUID}\r\n").into_bytes());
        assert_eq!(ev, Event::Begun);
    }

    #[test]
    fn overlong_unterminated_line_fails() {
        let mut s = sasl();
        let mut buf = vec![0u8];
        buf.extend(std::iter::repeat_n(b'a', MAX_LINE + 1));
        assert_eq!(s.feed(&mut buf).1, Event::Failed);
    }
}

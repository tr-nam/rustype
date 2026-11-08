//! D-Bus framing and header encoding for the bus and the IBus service. rustype_ibus::dbus
//! reads from a blocking stream and drops the destination field, so the server needs its own
//! parser: incremental framing from the fixed header, and re-encoding with sender/destination.

use rustype_ibus::dbus::{Message, Reader, Writer};
use std::io;

/// Largest message (header + body) accepted from, or queued for, one connection.
pub const MAX_FRAME: usize = 16 << 20;

pub fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn word(buf: &[u8], off: usize, big: bool) -> usize {
    let a = [buf[off], buf[off + 1], buf[off + 2], buf[off + 3]];
    (if big {
        u32::from_be_bytes(a)
    } else {
        u32::from_le_bytes(a)
    }) as usize
}

/// Total length of the first message in `buf`, or `None` while it is still partial.
pub fn frame_len(buf: &[u8]) -> io::Result<Option<usize>> {
    let Some(&endian) = buf.first() else {
        return Ok(None);
    };
    let big = match endian {
        b'l' => false,
        b'B' => true,
        _ => return Err(invalid("bad endianness byte")),
    };
    if buf.len() < 16 {
        return Ok(None);
    }
    let body_len = word(buf, 4, big);
    let fields_len = word(buf, 12, big);
    if body_len > MAX_FRAME || fields_len > MAX_FRAME {
        return Err(invalid("message too large"));
    }
    let total = ((16 + fields_len + 7) & !7) + body_len;
    if total > MAX_FRAME {
        return Err(invalid("message too large"));
    }
    Ok((buf.len() >= total).then_some(total))
}

/// A parsed message plus the destination header field, which `Message` does not carry.
pub struct Frame {
    pub msg: Message,
    pub dest: Option<String>,
    /// Original bytes of a big-endian message, which is forwarded untouched.
    raw: Option<Vec<u8>>,
}

/// Parses exactly one message (`all` must be the full frame reported by `frame_len`).
pub fn parse(all: &[u8]) -> io::Result<Frame> {
    if all.len() < 16 {
        return Err(invalid("short header"));
    }
    let big = all[0] == b'B';
    let header_end = 16 + word(all, 12, big);
    let body_start = (header_end + 7) & !7;
    if all.len() < body_start {
        return Err(invalid("short header"));
    }
    let mut m = Message {
        mtype: all[1],
        flags: all[2],
        serial: word(all, 8, big) as u32,
        big_endian: big,
        ..Default::default()
    };
    let mut dest = None;
    let mut r = Reader::new(&all[..header_end], big);
    r.pos = 16;
    while r.pos < header_end {
        r.align(8);
        if r.pos >= header_end {
            break;
        }
        let code = r.byte()?;
        let sig = r.signature()?;
        match (code, sig.as_str()) {
            (1, "o") => m.path = Some(r.string()?),
            (2, "s") => m.interface = Some(r.string()?),
            (3, "s") => m.member = Some(r.string()?),
            (4, "s") => m.error_name = Some(r.string()?),
            (5, "u") => m.reply_serial = Some(r.u32()?),
            (6, "s") => dest = Some(r.string()?),
            (7, "s") => m.sender = Some(r.string()?),
            (8, "g") => m.signature = r.signature()?,
            _ => {
                let mut i = 0;
                r.skip(sig.as_bytes(), &mut i)?;
            }
        }
    }
    m.body = all[body_start..].to_vec();
    let raw = big.then(|| all.to_vec());
    Ok(Frame { msg: m, dest, raw })
}

impl Frame {
    /// The message as bytes for another connection: little-endian with the sender set to the
    /// caller's unique name by the bus. Big-endian messages are forwarded byte for byte.
    pub fn bytes(&self) -> Vec<u8> {
        if let Some(raw) = &self.raw {
            return raw.clone();
        }
        let m = &self.msg;
        encode(
            m.mtype,
            m.flags,
            m.serial,
            Hdr {
                path: m.path.as_deref(),
                iface: m.interface.as_deref(),
                member: m.member.as_deref(),
                error: m.error_name.as_deref(),
                reply: m.reply_serial,
                dest: self.dest.as_deref(),
                sender: m.sender.as_deref(),
            },
            &m.signature,
            &m.body,
        )
    }
}

/// Header fields to encode. Absent fields are simply left out.
#[derive(Default, Clone, Copy)]
pub struct Hdr<'a> {
    pub path: Option<&'a str>,
    pub iface: Option<&'a str>,
    pub member: Option<&'a str>,
    pub error: Option<&'a str>,
    pub reply: Option<u32>,
    pub dest: Option<&'a str>,
    pub sender: Option<&'a str>,
}

fn field(w: &mut Writer, code: u8, ty: &str) {
    w.struct_start();
    w.byte(code);
    w.signature(ty);
}

/// Encodes one little-endian message.
pub fn encode(mtype: u8, flags: u8, serial: u32, h: Hdr, sig: &str, body: &[u8]) -> Vec<u8> {
    let mut w = Writer::new();
    w.byte(b'l');
    w.byte(mtype);
    w.byte(flags);
    w.byte(1);
    w.u32(body.len() as u32);
    w.u32(serial);
    let arr = w.array_start(8);
    if let Some(p) = h.path {
        field(&mut w, 1, "o");
        w.string(p);
    }
    if let Some(s) = h.iface {
        field(&mut w, 2, "s");
        w.string(s);
    }
    if let Some(s) = h.member {
        field(&mut w, 3, "s");
        w.string(s);
    }
    if let Some(s) = h.error {
        field(&mut w, 4, "s");
        w.string(s);
    }
    if let Some(u) = h.reply {
        field(&mut w, 5, "u");
        w.u32(u);
    }
    if let Some(s) = h.dest {
        field(&mut w, 6, "s");
        w.string(s);
    }
    if let Some(s) = h.sender {
        field(&mut w, 7, "s");
        w.string(s);
    }
    if !sig.is_empty() {
        field(&mut w, 8, "g");
        w.signature(sig);
    }
    w.array_end(arr);
    w.pad(8);
    w.buf.extend_from_slice(body);
    w.buf
}

/// Body encoders for the simple replies.
pub fn body_str(s: &str) -> Vec<u8> {
    let mut w = Writer::new();
    w.string(s);
    w.buf
}

pub fn body_u32(v: u32) -> Vec<u8> {
    let mut w = Writer::new();
    w.u32(v);
    w.buf
}

pub fn body_bool(v: bool) -> Vec<u8> {
    let mut w = Writer::new();
    w.boolean(v);
    w.buf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(7);
        w.string("xin chào");
        encode(
            1,
            0,
            42,
            Hdr {
                path: Some("/org/freedesktop/IBus"),
                iface: Some("org.freedesktop.IBus"),
                member: Some("Ping"),
                dest: Some("org.freedesktop.IBus"),
                sender: Some(":1.7"),
                ..Default::default()
            },
            "us",
            &w.buf,
        )
    }

    #[test]
    fn partial_reads_wait_for_the_whole_frame() {
        let bytes = sample();
        let total = bytes.len();
        // Every strict prefix is "not yet", byte by byte, including the header split points.
        for cut in 0..total {
            assert_eq!(frame_len(&bytes[..cut]).unwrap(), None, "cut at {cut}");
        }
        assert_eq!(frame_len(&bytes).unwrap(), Some(total));
    }

    #[test]
    fn two_messages_in_one_buffer_are_split_at_the_first_frame() {
        let mut bytes = sample();
        let first = bytes.len();
        bytes.extend(sample());
        assert_eq!(frame_len(&bytes).unwrap(), Some(first));
        let f = parse(&bytes[..first]).unwrap();
        assert_eq!(f.msg.serial, 42);
        assert_eq!(f.msg.member.as_deref(), Some("Ping"));
        assert_eq!(f.dest.as_deref(), Some("org.freedesktop.IBus"));
        assert_eq!(f.msg.sender.as_deref(), Some(":1.7"));
        let mut r = f.msg.body_reader();
        assert_eq!(r.u32().unwrap(), 7);
        assert_eq!(r.string().unwrap(), "xin chào");
    }

    #[test]
    fn parsed_frame_reencodes_identically_with_the_same_sender() {
        let bytes = sample();
        let f = parse(&bytes).unwrap();
        assert_eq!(f.bytes(), bytes);
    }

    #[test]
    fn oversized_and_bad_headers_are_rejected() {
        let mut h = vec![0u8; 16];
        h[0] = b'l';
        h[4..8].copy_from_slice(&(17u32 << 20).to_le_bytes());
        assert!(frame_len(&h).is_err());
        let mut bad = vec![0u8; 16];
        bad[0] = b'x';
        assert!(frame_len(&bad).is_err());
    }

    #[test]
    fn big_endian_header_is_framed() {
        // Fixed header only: endianness B, body 4 bytes, no header fields.
        let mut h = vec![b'B', 2, 0, 1];
        h.extend_from_slice(&4u32.to_be_bytes());
        h.extend_from_slice(&9u32.to_be_bytes());
        h.extend_from_slice(&0u32.to_be_bytes());
        assert_eq!(frame_len(&h).unwrap(), None);
        h.extend_from_slice(&[0, 0, 0, 1]);
        assert_eq!(frame_len(&h).unwrap(), Some(20));
        let f = parse(&h).unwrap();
        assert!(f.msg.big_endian);
        assert_eq!(f.msg.serial, 9);
        assert_eq!(f.bytes(), h);
    }
}

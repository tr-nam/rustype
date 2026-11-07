//! A small D-Bus client: just enough of the wire protocol for an IBus engine.
//! Unix socket, SASL EXTERNAL, little-endian messages out, either endianness in.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;

pub const METHOD_CALL: u8 = 1;
pub const METHOD_RETURN: u8 = 2;
pub const ERROR: u8 = 3;
pub const SIGNAL: u8 = 4;
pub const FLAG_NO_REPLY_EXPECTED: u8 = 0x1;

const MAX_MESSAGE: usize = 64 << 20;

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

// ---------------------------------------------------------------- writing

/// Marshals values with D-Bus alignment. Offsets are relative to the start of `buf`,
/// which is fine for bodies because a body always starts on an 8-byte boundary.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

pub struct ArrayMark {
    len_at: usize,
    start: usize,
}

impl Writer {
    pub fn new() -> Self {
        Writer { buf: Vec::with_capacity(128) }
    }
    pub fn pad(&mut self, align: usize) {
        let rem = self.buf.len() % align;
        if rem != 0 {
            self.buf.resize(self.buf.len() + align - rem, 0);
        }
    }
    pub fn byte(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u32(&mut self, v: u32) {
        self.pad(4);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.pad(4);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn boolean(&mut self, v: bool) {
        self.u32(v as u32);
    }
    pub fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }
    pub fn signature(&mut self, s: &str) {
        self.buf.push(s.len() as u8);
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }
    pub fn struct_start(&mut self) {
        self.pad(8);
    }
    pub fn array_start(&mut self, elem_align: usize) -> ArrayMark {
        self.pad(4);
        let len_at = self.buf.len();
        self.buf.extend_from_slice(&[0; 4]);
        self.pad(elem_align);
        ArrayMark { len_at, start: self.buf.len() }
    }
    pub fn array_end(&mut self, m: ArrayMark) {
        let len = (self.buf.len() - m.start) as u32;
        self.buf[m.len_at..m.len_at + 4].copy_from_slice(&len.to_le_bytes());
    }
}

pub enum Field<'a> {
    Path(&'a str),
    Interface(&'a str),
    Member(&'a str),
    ErrorName(&'a str),
    ReplySerial(u32),
    Destination(&'a str),
}

pub fn encode(mtype: u8, flags: u8, serial: u32, fields: &[Field], sig: &str, body: &[u8]) -> Vec<u8> {
    let mut w = Writer::new();
    w.byte(b'l');
    w.byte(mtype);
    w.byte(flags);
    w.byte(1);
    w.u32(body.len() as u32);
    w.u32(serial);
    let arr = w.array_start(8);
    let field = |w: &mut Writer, code: u8, ty: &str| {
        w.struct_start();
        w.byte(code);
        w.signature(ty);
    };
    for f in fields {
        match *f {
            Field::Path(p) => {
                field(&mut w, 1, "o");
                w.string(p);
            }
            Field::Interface(s) => {
                field(&mut w, 2, "s");
                w.string(s);
            }
            Field::Member(s) => {
                field(&mut w, 3, "s");
                w.string(s);
            }
            Field::ErrorName(s) => {
                field(&mut w, 4, "s");
                w.string(s);
            }
            Field::ReplySerial(u) => {
                field(&mut w, 5, "u");
                w.u32(u);
            }
            Field::Destination(s) => {
                field(&mut w, 6, "s");
                w.string(s);
            }
        }
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

// ---------------------------------------------------------------- reading

pub struct Reader<'a> {
    b: &'a [u8],
    pub pos: usize,
    big: bool,
}

fn type_align(t: u8) -> usize {
    match t {
        b'n' | b'q' => 2,
        b'b' | b'i' | b'u' | b'h' | b's' | b'o' | b'a' => 4,
        b'x' | b't' | b'd' | b'(' | b'{' => 8,
        _ => 1, // y g v
    }
}

/// Advances `i` past one complete type in a signature.
fn sig_skip(sig: &[u8], i: &mut usize) -> io::Result<()> {
    let t = *sig.get(*i).ok_or_else(|| bad("signature ended early"))?;
    *i += 1;
    match t {
        b'a' => sig_skip(sig, i),
        b'(' | b'{' => {
            let close = if t == b'(' { b')' } else { b'}' };
            while *sig.get(*i).ok_or_else(|| bad("unclosed container"))? != close {
                sig_skip(sig, i)?;
            }
            *i += 1;
            Ok(())
        }
        _ => Ok(()),
    }
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8], big: bool) -> Self {
        Reader { b, pos: 0, big }
    }
    pub fn align(&mut self, a: usize) {
        self.pos = (self.pos + a - 1) / a * a;
    }
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(|| bad("message too short"))?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    pub fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u32(&mut self) -> io::Result<u32> {
        self.align(4);
        let s: [u8; 4] = self.take(4)?.try_into().unwrap();
        Ok(if self.big { u32::from_be_bytes(s) } else { u32::from_le_bytes(s) })
    }
    pub fn string(&mut self) -> io::Result<String> {
        let n = self.u32()? as usize;
        let s = self.take(n)?;
        self.take(1)?;
        String::from_utf8(s.to_vec()).map_err(|_| bad("string is not UTF-8"))
    }
    pub fn signature(&mut self) -> io::Result<String> {
        let n = self.byte()? as usize;
        let s = self.take(n)?;
        self.take(1)?;
        String::from_utf8(s.to_vec()).map_err(|_| bad("bad signature"))
    }
    /// Skips one value whose type starts at `sig[*i]`.
    pub fn skip(&mut self, sig: &[u8], i: &mut usize) -> io::Result<()> {
        let t = *sig.get(*i).ok_or_else(|| bad("signature ended early"))?;
        match t {
            b'y' => {
                self.take(1)?;
                *i += 1;
            }
            b'n' | b'q' => {
                self.align(2);
                self.take(2)?;
                *i += 1;
            }
            b'b' | b'i' | b'u' | b'h' => {
                self.u32()?;
                *i += 1;
            }
            b'x' | b't' | b'd' => {
                self.align(8);
                self.take(8)?;
                *i += 1;
            }
            b's' | b'o' => {
                self.string()?;
                *i += 1;
            }
            b'g' => {
                self.signature()?;
                *i += 1;
            }
            b'v' => {
                let inner = self.signature()?;
                let mut j = 0;
                self.skip(inner.as_bytes(), &mut j)?;
                *i += 1;
            }
            b'a' => {
                let len = self.u32()? as usize;
                let elem = *sig.get(*i + 1).ok_or_else(|| bad("array without element type"))?;
                self.align(type_align(elem));
                self.take(len)?;
                *i += 1;
                sig_skip(sig, i)?;
            }
            b'(' | b'{' => {
                self.align(8);
                let close = if t == b'(' { b')' } else { b'}' };
                *i += 1;
                while *sig.get(*i).ok_or_else(|| bad("unclosed container"))? != close {
                    self.skip(sig, i)?;
                }
                *i += 1;
            }
            _ => return Err(bad("unknown type code")),
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct Message {
    pub mtype: u8,
    pub flags: u8,
    pub serial: u32,
    pub path: Option<String>,
    pub interface: Option<String>,
    pub member: Option<String>,
    pub error_name: Option<String>,
    pub reply_serial: Option<u32>,
    pub sender: Option<String>,
    pub signature: String,
    pub body: Vec<u8>,
    pub big_endian: bool,
}

impl Message {
    pub fn body_reader(&self) -> Reader<'_> {
        Reader::new(&self.body, self.big_endian)
    }
    pub fn wants_reply(&self) -> bool {
        self.flags & FLAG_NO_REPLY_EXPECTED == 0
    }
}

pub fn read_message(r: &mut impl Read) -> io::Result<Message> {
    let mut fixed = [0u8; 16];
    r.read_exact(&mut fixed)?;
    let big = match fixed[0] {
        b'l' => false,
        b'B' => true,
        _ => return Err(bad("bad endianness byte")),
    };
    let rd = |s: &[u8]| {
        let a: [u8; 4] = s.try_into().unwrap();
        if big { u32::from_be_bytes(a) } else { u32::from_le_bytes(a) }
    };
    let body_len = rd(&fixed[4..8]) as usize;
    let fields_len = rd(&fixed[12..16]) as usize;
    if body_len > MAX_MESSAGE || fields_len > MAX_MESSAGE {
        return Err(bad("message too large"));
    }
    let header_end = 16 + fields_len;
    let body_start = (header_end + 7) & !7;
    let mut all = vec![0u8; body_start + body_len];
    all[..16].copy_from_slice(&fixed);
    r.read_exact(&mut all[16..])?;

    let mut m = Message { mtype: fixed[1], flags: fixed[2], serial: rd(&fixed[8..12]), big_endian: big, ..Default::default() };
    let mut rd = Reader::new(&all[..header_end], big);
    rd.pos = 16;
    while rd.pos < header_end {
        rd.align(8);
        if rd.pos >= header_end {
            break;
        }
        let code = rd.byte()?;
        let sig = rd.signature()?;
        match (code, sig.as_str()) {
            (1, "o") => m.path = Some(rd.string()?),
            (2, "s") => m.interface = Some(rd.string()?),
            (3, "s") => m.member = Some(rd.string()?),
            (4, "s") => m.error_name = Some(rd.string()?),
            (5, "u") => m.reply_serial = Some(rd.u32()?),
            (7, "s") => m.sender = Some(rd.string()?),
            (8, "g") => m.signature = rd.signature()?,
            _ => {
                let mut i = 0;
                rd.skip(sig.as_bytes(), &mut i)?;
            }
        }
    }
    m.body = all.split_off(body_start);
    Ok(m)
}

// ---------------------------------------------------------------- connection

/// Splits a D-Bus address and connects to the first unix transport in it.
fn connect_address(address: &str) -> io::Result<UnixStream> {
    let mut last_err = bad("no usable unix address");
    for entry in address.split(';') {
        let Some(params) = entry.strip_prefix("unix:") else { continue };
        for kv in params.split(',') {
            let Some((k, v)) = kv.split_once('=') else { continue };
            let v = unescape(v);
            let res = match k {
                "path" => UnixStream::connect(&v),
                "abstract" => {
                    use std::os::linux::net::SocketAddrExt;
                    std::os::unix::net::SocketAddr::from_abstract_name(v.as_bytes())
                        .and_then(|a| UnixStream::connect_addr(&a))
                }
                _ => continue,
            };
            match res {
                Ok(s) => return Ok(s),
                Err(e) => last_err = e,
            }
        }
    }
    Err(last_err)
}

fn unescape(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn current_uid() -> io::Result<u32> {
    use std::os::unix::fs::MetadataExt;
    Ok(std::fs::metadata("/proc/self")?.uid())
}

pub struct Connection {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    serial: u32,
    queued: std::collections::VecDeque<Message>,
    pub unique_name: String,
}

impl Connection {
    pub fn open(address: &str) -> io::Result<Connection> {
        let stream = connect_address(address)?;
        let writer = stream.try_clone()?;
        let mut c = Connection {
            reader: BufReader::new(stream),
            writer,
            serial: 0,
            queued: Default::default(),
            unique_name: String::new(),
        };
        c.authenticate()?;
        let reply = c.call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "Hello", "", &[])?;
        c.unique_name = reply.body_reader().string()?;
        Ok(c)
    }

    fn authenticate(&mut self) -> io::Result<()> {
        let uid_hex: String = current_uid()?.to_string().bytes().map(|b| format!("{b:02x}")).collect();
        self.writer.write_all(format!("\0AUTH EXTERNAL {uid_hex}\r\n").as_bytes())?;
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        if !line.starts_with("OK ") {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("D-Bus auth failed: {}", line.trim())));
        }
        self.writer.write_all(b"BEGIN\r\n")
    }

    fn next_serial(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1).max(1);
        self.serial
    }

    /// Sends a method call and waits for its reply; other messages are queued for `next`.
    pub fn call(&mut self, dest: &str, path: &str, iface: &str, member: &str, sig: &str, body: &[u8]) -> io::Result<Message> {
        let serial = self.next_serial();
        let fields = [Field::Path(path), Field::Interface(iface), Field::Member(member), Field::Destination(dest)];
        self.writer.write_all(&encode(METHOD_CALL, 0, serial, &fields, sig, body))?;
        loop {
            let m = read_message(&mut self.reader)?;
            if matches!(m.mtype, METHOD_RETURN | ERROR) && m.reply_serial == Some(serial) {
                if m.mtype == ERROR {
                    let text = if m.signature.starts_with('s') { m.body_reader().string().unwrap_or_default() } else { String::new() };
                    return Err(io::Error::other(format!("{}: {}", m.error_name.unwrap_or_default(), text)));
                }
                return Ok(m);
            }
            self.queued.push_back(m);
        }
    }

    pub fn request_name(&mut self, name: &str) -> io::Result<u32> {
        let mut w = Writer::new();
        w.string(name);
        w.u32(0x4); // DBUS_NAME_FLAG_DO_NOT_QUEUE
        let reply = self.call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", "su", &w.buf)?;
        reply.body_reader().u32()
    }

    pub fn next(&mut self) -> io::Result<Message> {
        if let Some(m) = self.queued.pop_front() {
            return Ok(m);
        }
        read_message(&mut self.reader)
    }

    /// Appends an encoded signal to `batch`.
    pub fn signal(&mut self, batch: &mut Vec<u8>, path: &str, iface: &str, member: &str, sig: &str, body: &[u8]) {
        let serial = self.next_serial();
        let fields = [Field::Path(path), Field::Interface(iface), Field::Member(member)];
        batch.extend(encode(SIGNAL, 0, serial, &fields, sig, body));
    }

    /// Appends the reply for `call` to `batch` (nothing when no reply is expected).
    pub fn reply(&mut self, batch: &mut Vec<u8>, call: &Message, sig: &str, body: &[u8]) {
        if !call.wants_reply() {
            return;
        }
        let serial = self.next_serial();
        let mut fields = vec![Field::ReplySerial(call.serial)];
        if let Some(s) = &call.sender {
            fields.push(Field::Destination(s));
        }
        batch.extend(encode(METHOD_RETURN, 0, serial, &fields, sig, body));
    }

    pub fn error(&mut self, batch: &mut Vec<u8>, call: &Message, name: &str, text: &str) {
        if !call.wants_reply() {
            return;
        }
        let serial = self.next_serial();
        let mut fields = vec![Field::ReplySerial(call.serial), Field::ErrorName(name)];
        if let Some(s) = &call.sender {
            fields.push(Field::Destination(s));
        }
        let mut w = Writer::new();
        w.string(text);
        batch.extend(encode(ERROR, 0, serial, &fields, "s", &w.buf));
    }

    /// One write for signals + reply keeps them in order and costs a single syscall.
    pub fn flush(&mut self, batch: &[u8]) -> io::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        self.writer.write_all(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_header_and_body() {
        let mut w = Writer::new();
        w.u32(7);
        w.string("xin chào");
        let bytes = encode(
            METHOD_CALL,
            0,
            42,
            &[Field::Path("/a/b"), Field::Interface("x.y"), Field::Member("M"), Field::Destination("d.e")],
            "us",
            &w.buf,
        );
        let m = read_message(&mut &bytes[..]).unwrap();
        assert_eq!(m.serial, 42);
        assert_eq!(m.path.as_deref(), Some("/a/b"));
        assert_eq!(m.interface.as_deref(), Some("x.y"));
        assert_eq!(m.member.as_deref(), Some("M"));
        assert_eq!(m.signature, "us");
        let mut r = m.body_reader();
        assert_eq!(r.u32().unwrap(), 7);
        assert_eq!(r.string().unwrap(), "xin chào");
    }

    #[test]
    fn skip_nested_values() {
        // (sa{sv}sv) with an empty dict and a variant holding (sa{sv}av)
        let mut w = Writer::new();
        w.struct_start();
        w.string("IBusText");
        let a = w.array_start(8);
        w.array_end(a);
        w.string("abc");
        w.signature("(sa{sv}av)");
        w.struct_start();
        w.string("IBusAttrList");
        let a = w.array_start(8);
        w.array_end(a);
        let a = w.array_start(1);
        w.array_end(a);
        let end = w.buf.len();
        w.u32(99);
        let mut r = Reader::new(&w.buf, false);
        let mut i = 0;
        r.skip(b"(sa{sv}sv)", &mut i).unwrap();
        assert_eq!(r.pos, end);
        assert_eq!(r.u32().unwrap(), 99);
    }

    #[test]
    fn unescape_address() {
        assert_eq!(unescape("/tmp/a%20b"), "/tmp/a b");
        assert_eq!(unescape("/plain/path"), "/plain/path");
    }
}

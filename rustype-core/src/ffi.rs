//! C FFI for rustype-core: allows Fcitx5, Windows TSF, and macOS InputMethodKit
//! to embed the Vietnamese Telex engine directly via C ABI.

use crate::telex::{Config, Telex};
use std::ffi::{c_char, CStr};
use std::ptr;

pub struct RustypeTelex {
    inner: Telex,
}

#[no_mangle]
pub unsafe extern "C" fn rustype_telex_new(modern_orthography: bool, auto_restore_english: bool) -> *mut RustypeTelex {
    let cfg = Config {
        modern_orthography,
        auto_restore_english,
    };
    Box::into_raw(Box::new(RustypeTelex {
        inner: Telex::new(cfg),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rustype_telex_free(telex: *mut RustypeTelex) {
    if !telex.is_null() {
        drop(Box::from_raw(telex));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rustype_telex_reset(telex: *mut RustypeTelex) {
    if let Some(t) = telex.as_mut() {
        t.inner.reset();
    }
}

unsafe fn write_c_string(text: &str, out_buf: *mut c_char, max_len: usize) {
    if out_buf.is_null() || max_len == 0 {
        return;
    }
    let bytes = text.as_bytes();
    let copy_len = bytes.len().min(max_len - 1);
    ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, out_buf, copy_len);
    *out_buf.add(copy_len) = 0;
}

#[no_mangle]
pub unsafe extern "C" fn rustype_telex_key(
    telex: *mut RustypeTelex,
    ch: u32,
    out_handled: *mut bool,
    out_backspaces: *mut usize,
    out_commit: *mut c_char,
    out_commit_capacity: usize,
) -> bool {
    let Some(t) = telex.as_mut() else { return false };
    let Some(c) = char::from_u32(ch) else { return false };
    let res = t.inner.key(c);
    if !out_handled.is_null() {
        *out_handled = res.handled;
    }
    if !out_backspaces.is_null() {
        *out_backspaces = res.backspaces;
    }
    write_c_string(&res.commit, out_commit, out_commit_capacity);
    res.handled
}

#[no_mangle]
pub unsafe extern "C" fn rustype_telex_backspace(
    telex: *mut RustypeTelex,
    out_handled: *mut bool,
    out_backspaces: *mut usize,
    out_commit: *mut c_char,
    out_commit_capacity: usize,
) -> bool {
    let Some(t) = telex.as_mut() else { return false };
    let res = t.inner.backspace();
    if !out_handled.is_null() {
        *out_handled = res.handled;
    }
    if !out_backspaces.is_null() {
        *out_backspaces = res.backspaces;
    }
    write_c_string(&res.commit, out_commit, out_commit_capacity);
    res.handled
}

#[no_mangle]
pub unsafe extern "C" fn rustype_telex_feed(
    telex: *mut RustypeTelex,
    input: *const c_char,
    out_buf: *mut c_char,
    out_buf_len: usize,
) -> usize {
    let Some(t) = telex.as_mut() else { return 0 };
    if input.is_null() {
        return 0;
    }
    let Ok(cstr) = CStr::from_ptr(input).to_str() else { return 0 };
    let result = t.inner.feed(cstr);
    write_c_string(&result, out_buf, out_buf_len);
    result.len()
}

#[no_mangle]
pub extern "C" fn rustype_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn ffi_lifecycle_and_feed() {
        unsafe {
            let t = rustype_telex_new(false, true);
            assert!(!t.is_null());

            let input = CString::new("Vieetj Nam").unwrap();
            let mut out = [0 as c_char; 128];
            let len = rustype_telex_feed(t, input.as_ptr(), out.as_mut_ptr(), out.len());
            assert!(len > 0);

            let res = CStr::from_ptr(out.as_ptr()).to_str().unwrap();
            assert_eq!(res, "Việt Nam");

            rustype_telex_reset(t);
            rustype_telex_free(t);
        }
    }

    #[test]
    fn ffi_keys_and_backspace() {
        unsafe {
            let t = rustype_telex_new(false, true);
            let mut handled = false;
            let mut backspaces = 0;
            let mut commit = [0 as c_char; 64];

            // Type 'w' -> 'ư'
            let h = rustype_telex_key(t, 'w' as u32, &mut handled, &mut backspaces, commit.as_mut_ptr(), commit.len());
            assert!(h);
            assert_eq!(backspaces, 0);
            let s = CStr::from_ptr(commit.as_ptr()).to_str().unwrap();
            assert_eq!(s, "ư");

            // Backspace
            let h = rustype_telex_backspace(t, &mut handled, &mut backspaces, commit.as_mut_ptr(), commit.len());
            assert!(!h); // empty now, passes to app

            rustype_telex_free(t);
        }
    }
}

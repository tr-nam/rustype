//! C FFI for rustype-core: allows C-ABI input method frameworks, such as macOS InputMethodKit
//! and Windows TSF, to embed the Vietnamese Telex engine directly via C ABI.

use crate::telex::{Config, Telex};
use std::ffi::{c_char, CStr};
use std::ptr;

pub struct RustypeTelex {
    inner: Telex,
}

/// Creates a new `RustypeTelex` instance.
///
/// # Safety
/// The returned pointer must be freed with [`rustype_telex_free`].
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_new(
    modern_orthography: bool,
    auto_restore_english: bool,
) -> *mut RustypeTelex {
    let cfg = Config {
        modern_orthography,
        auto_restore_english,
        ..Default::default()
    };
    Box::into_raw(Box::new(RustypeTelex {
        inner: Telex::new(cfg),
    }))
}

/// Frees a `RustypeTelex` instance previously allocated with [`rustype_telex_new`].
///
/// # Safety
/// `telex` must be a valid pointer allocated by [`rustype_telex_new`] or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_free(telex: *mut RustypeTelex) {
    if !telex.is_null() {
        drop(Box::from_raw(telex));
    }
}

/// Resets the engine composition state.
///
/// # Safety
/// `telex` must be a valid pointer to a [`RustypeTelex`] instance or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_reset(telex: *mut RustypeTelex) {
    if let Some(t) = telex.as_mut() {
        t.inner.reset();
    }
}

/// Sets the input method: 0 = Telex, 1 = VNI.
///
/// # Safety
/// `telex` must be a valid pointer or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_set_input_method(telex: *mut RustypeTelex, method: u32) {
    if let Some(t) = telex.as_mut() {
        t.inner.cfg.input_method = match method {
            1 => crate::telex::InputMethod::Vni,
            _ => crate::telex::InputMethod::Telex,
        };
    }
}

/// Configures modern orthography (false: hòa, khỏe; true: hoà, khoẻ).
///
/// # Safety
/// `telex` must be a valid pointer or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_set_modern_orthography(
    telex: *mut RustypeTelex,
    modern: bool,
) {
    if let Some(t) = telex.as_mut() {
        t.inner.cfg.modern_orthography = modern;
    }
}

/// Configures automatic restoration of English words on word boundaries.
///
/// # Safety
/// `telex` must be a valid pointer or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_set_auto_restore_english(
    telex: *mut RustypeTelex,
    enable: bool,
) {
    if let Some(t) = telex.as_mut() {
        t.inner.cfg.auto_restore_english = enable;
    }
}

/// Enables or disables macro expansion.
///
/// # Safety
/// `telex` must be a valid pointer or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_set_macro_enabled(telex: *mut RustypeTelex, enable: bool) {
    if let Some(t) = telex.as_mut() {
        t.inner.cfg.enable_macro = enable;
    }
}

/// Adds a macro shortcut mapping.
///
/// # Safety
/// `telex`, `key`, and `value` must be valid non-null C strings.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_add_macro(
    telex: *mut RustypeTelex,
    key: *const c_char,
    value: *const c_char,
) {
    let Some(t) = telex.as_mut() else { return };
    if key.is_null() || value.is_null() {
        return;
    }
    if let (Ok(k), Ok(v)) = (CStr::from_ptr(key).to_str(), CStr::from_ptr(value).to_str()) {
        t.inner.cfg.macros.insert(k.to_string(), v.to_string());
    }
}

/// Clears all macro shortcut mappings.
///
/// # Safety
/// `telex` must be a valid pointer or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_clear_macros(telex: *mut RustypeTelex) {
    if let Some(t) = telex.as_mut() {
        t.inner.cfg.macros.clear();
    }
}

/// Adds an English/foreign custom word to avoid accents.
///
/// # Safety
/// `telex` and `word` must be valid non-null C strings.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_add_custom_word(
    telex: *mut RustypeTelex,
    word: *const c_char,
) {
    let Some(t) = telex.as_mut() else { return };
    if word.is_null() {
        return;
    }
    if let Ok(w) = CStr::from_ptr(word).to_str() {
        let ws = w.to_string();
        if !t.inner.cfg.custom_words.contains(&ws) {
            t.inner.cfg.custom_words.push(ws);
        }
    }
}

/// Clears all custom words.
///
/// # Safety
/// `telex` must be a valid pointer or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_clear_custom_words(telex: *mut RustypeTelex) {
    if let Some(t) = telex.as_mut() {
        t.inner.cfg.custom_words.clear();
    }
}

/// Checks if the composition buffer is empty.
///
/// # Safety
/// `telex` must be a valid pointer or null.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_is_empty(telex: *mut RustypeTelex) -> bool {
    let Some(t) = telex.as_mut() else { return true };
    t.inner.is_empty()
}

/// Gets the currently displayed preedit composition text.
///
/// # Safety
/// `telex` must be a valid pointer or null. `out_buf` must be writable.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_get_displayed(
    telex: *mut RustypeTelex,
    out_buf: *mut c_char,
    out_buf_len: usize,
) -> usize {
    let Some(t) = telex.as_mut() else { return 0 };
    let text = t.inner.displayed();
    write_c_string(&text, out_buf, out_buf_len)
}

/// Copies `text` into a NUL-terminated buffer of `max_len` bytes. Longer text is cut at the
/// last UTF-8 character boundary that fits. Returns the bytes copied, excluding the NUL.
unsafe fn write_c_string(text: &str, out_buf: *mut c_char, max_len: usize) -> usize {
    if out_buf.is_null() || max_len == 0 {
        return 0;
    }
    let mut copy_len = text.len().min(max_len - 1);
    while !text.is_char_boundary(copy_len) {
        copy_len -= 1;
    }
    ptr::copy_nonoverlapping(text.as_ptr() as *const c_char, out_buf, copy_len);
    *out_buf.add(copy_len) = 0;
    copy_len
}

/// Processes a single unicode character input.
///
/// # Safety
/// `telex` must be a valid pointer or null. Output pointers, if non-null, must be writable.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_key(
    telex: *mut RustypeTelex,
    ch: u32,
    out_handled: *mut bool,
    out_backspaces: *mut usize,
    out_commit: *mut c_char,
    out_commit_capacity: usize,
) -> bool {
    let Some(t) = telex.as_mut() else {
        return false;
    };
    let Some(c) = char::from_u32(ch) else {
        return false;
    };
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

/// Processes a backspace key.
///
/// # Safety
/// `telex` must be a valid pointer or null. Output pointers, if non-null, must be writable.
#[no_mangle]
pub unsafe extern "C" fn rustype_telex_backspace(
    telex: *mut RustypeTelex,
    out_handled: *mut bool,
    out_backspaces: *mut usize,
    out_commit: *mut c_char,
    out_commit_capacity: usize,
) -> bool {
    let Some(t) = telex.as_mut() else {
        return false;
    };
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

/// Feeds an entire string into the engine and returns the converted text.
///
/// # Safety
/// `telex` must be a valid pointer or null. `input` must point to a valid null-terminated C string.
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
    let Ok(cstr) = CStr::from_ptr(input).to_str() else {
        return 0;
    };
    let result = t.inner.feed(cstr);
    write_c_string(&result, out_buf, out_buf_len)
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
            let h = rustype_telex_key(
                t,
                'w' as u32,
                &mut handled,
                &mut backspaces,
                commit.as_mut_ptr(),
                commit.len(),
            );
            assert!(h);
            assert_eq!(backspaces, 0);
            let s = CStr::from_ptr(commit.as_ptr()).to_str().unwrap();
            assert_eq!(s, "ư");

            // Backspace
            let h = rustype_telex_backspace(
                t,
                &mut handled,
                &mut backspaces,
                commit.as_mut_ptr(),
                commit.len(),
            );
            assert!(!h); // empty now, passes to app

            rustype_telex_free(t);
        }
    }

    #[test]
    fn ffi_truncates_on_char_boundary() {
        unsafe {
            let t = rustype_telex_new(false, true);
            // "Việt" is 6 bytes: V, i, ệ (3 bytes), t.
            let input = CString::new("Vieetj").unwrap();
            let mut out = [0x7f as c_char; 8];
            for (cap, want) in [
                (0usize, ""),
                (1, ""),
                (2, "V"),
                (3, "Vi"),
                (4, "Vi"),
                (5, "Vi"),
                (6, "Việ"),
                (7, "Việt"),
            ] {
                let n = rustype_telex_feed(t, input.as_ptr(), out.as_mut_ptr(), cap);
                assert_eq!(n, want.len(), "capacity {cap}");
                if cap > 0 {
                    let got = CStr::from_ptr(out.as_ptr()).to_str().unwrap();
                    assert_eq!(got, want, "capacity {cap}");
                }
            }

            // get_displayed: the composition buffer still holds "Việt".
            let n = rustype_telex_get_displayed(t, out.as_mut_ptr(), 5);
            assert_eq!(n, 2);
            assert_eq!(CStr::from_ptr(out.as_ptr()).to_str().unwrap(), "Vi");
            let n = rustype_telex_get_displayed(t, out.as_mut_ptr(), 7);
            assert_eq!(n, 6);
            assert_eq!(CStr::from_ptr(out.as_ptr()).to_str().unwrap(), "Việt");

            rustype_telex_free(t);
        }
    }

    #[test]
    fn ffi_vni_and_options() {
        unsafe {
            let t = rustype_telex_new(false, true);
            rustype_telex_set_input_method(t, 1); // VNI

            let input = CString::new("Viet65 Nam").unwrap();
            let mut out = [0 as c_char; 128];
            let len = rustype_telex_feed(t, input.as_ptr(), out.as_mut_ptr(), out.len());
            assert!(len > 0);
            let res = CStr::from_ptr(out.as_ptr()).to_str().unwrap();
            assert_eq!(res, "Việt Nam");

            // Macro
            rustype_telex_set_macro_enabled(t, true);
            let k = CString::new("hn").unwrap();
            let v = CString::new("Hà Nội").unwrap();
            rustype_telex_add_macro(t, k.as_ptr(), v.as_ptr());

            let mut out_m = [0 as c_char; 128];
            let input_m = CString::new("hn ").unwrap();
            rustype_telex_feed(t, input_m.as_ptr(), out_m.as_mut_ptr(), out_m.len());
            let res_m = CStr::from_ptr(out_m.as_ptr()).to_str().unwrap();
            assert_eq!(res_m, "Hà Nội ");

            rustype_telex_free(t);
        }
    }
}

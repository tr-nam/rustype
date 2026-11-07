//! Telex composition core: pure, no I/O.
//!
//! Tone placement, phonotactic checks, auto-restore of English words, double-key escapes
//! and backspace editing. Expected behaviour is pinned by tests/golden.tsv.

use crate::tables::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMethod {
    Telex,
    Vni,
}

impl Default for InputMethod {
    fn default() -> Self {
        InputMethod::Telex
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    /// false: hòa, khỏe, thúy (phổ biến). true: hoà, khoẻ, thuý (kiểu mới).
    pub modern_orthography: bool,
    /// Turn words that are not valid Vietnamese back into the raw keys on the word boundary.
    pub auto_restore_english: bool,
    /// Input method: Telex or VNI.
    pub input_method: InputMethod,
    /// Additional English/foreign words to never add accents to.
    pub custom_words: Vec<String>,
    /// Abbreviations to expand on word boundary (macro table).
    pub macros: HashMap<String, String>,
    /// Whether macro expansion is enabled.
    pub enable_macro: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            modern_orthography: false,
            auto_restore_english: true,
            input_method: InputMethod::Telex,
            custom_words: Vec::new(),
            macros: HashMap::new(),
            enable_macro: false,
        }
    }
}

/// What the app must do after a key: delete `backspaces` chars before the cursor,
/// then insert `commit`. When `handled` is false the app handles the key itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyResult {
    pub handled: bool,
    pub backspaces: usize,
    pub commit: String,
}

impl KeyResult {
    fn pass() -> Self {
        KeyResult { handled: false, backspaces: 0, commit: String::new() }
    }
}

#[derive(Clone, Debug)]
struct PrevWord {
    raw: Vec<char>,
    chars: Vec<char>,
    tone: u8,
    disabled: bool,
}

struct Split {
    initial: String,
    vowel: String,
    fin: String,
    vidx: Vec<usize>,
}

pub struct Telex {
    pub cfg: Config,
    raw: Vec<char>,
    chars: Vec<char>,
    tone: u8,
    disabled: bool,
    has_backspaced: bool,
    last_mod: Option<char>,
    prev: Option<PrevWord>,
    prev_trailing: usize,
}

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn upper(c: char) -> char {
    c.to_uppercase().next().unwrap_or(c)
}

/// `target` (lowercase) with the case of `like`.
fn cased(target: char, like: char) -> char {
    if like.is_uppercase() {
        upper(target)
    } else {
        target
    }
}

fn is_base_vowel(c: char) -> bool {
    matches!(c, 'a' | 'ă' | 'â' | 'e' | 'ê' | 'i' | 'o' | 'ô' | 'ơ' | 'u' | 'ư' | 'y')
}

fn is_modified_vowel(c: char) -> bool {
    matches!(c, 'ă' | 'â' | 'ê' | 'ô' | 'ơ' | 'ư')
}

fn in_set(set: &[&str], s: &str) -> bool {
    set.contains(&s)
}

fn tone_form(base: char, tone: u8) -> char {
    VOWEL_TABLE
        .iter()
        .find(|(b, _)| *b == base)
        .map(|(_, forms)| forms[tone as usize])
        .unwrap_or(base)
}

fn hat(c: char) -> Option<char> {
    Some(match c {
        'a' => 'â',
        'A' => 'Â',
        'e' => 'ê',
        'E' => 'Ê',
        'o' => 'ô',
        'O' => 'Ô',
        _ => return None,
    })
}

fn unhat(c: char) -> Option<char> {
    Some(match c {
        'â' => 'a',
        'Â' => 'A',
        'ê' => 'e',
        'Ê' => 'E',
        'ô' => 'o',
        'Ô' => 'O',
        _ => return None,
    })
}

fn unw(c: char) -> Option<char> {
    Some(match c {
        'ă' => 'a',
        'Ă' => 'A',
        'ơ' => 'o',
        'Ơ' => 'O',
        'ư' => 'u',
        'Ư' => 'U',
        _ => return None,
    })
}

fn tone_key(c: char) -> Option<u8> {
    Some(match c {
        's' => 1,
        'f' => 2,
        'r' => 3,
        'x' => 4,
        'j' => 5,
        'z' => 0,
        _ => return None,
    })
}

impl Telex {
    pub fn new(cfg: Config) -> Self {
        Telex {
            cfg,
            raw: Vec::new(),
            chars: Vec::new(),
            tone: 0,
            disabled: false,
            has_backspaced: false,
            last_mod: None,
            prev: None,
            prev_trailing: 0,
        }
    }

    /// Full reset (cursor moved, focus changed): also forgets the last committed word.
    pub fn reset(&mut self) {
        self.clear_word();
        self.prev = None;
        self.prev_trailing = 0;
    }

    fn clear_word(&mut self) {
        self.raw.clear();
        self.chars.clear();
        self.tone = 0;
        self.disabled = false;
        self.has_backspaced = false;
        self.last_mod = None;
    }

    fn commit_word(&mut self) {
        if self.chars.is_empty() {
            self.reset();
            return;
        }
        self.prev = Some(PrevWord {
            raw: self.raw.clone(),
            chars: self.chars.clone(),
            tone: self.tone,
            disabled: self.disabled,
        });
        self.prev_trailing = 1;
        self.clear_word();
    }

    fn restore_prev_word(&mut self) {
        if let Some(p) = self.prev.take() {
            self.raw = p.raw;
            self.chars = p.chars;
            self.tone = p.tone;
            self.disabled = p.disabled;
            // Editing a finished word: never auto-restore it to English on the next boundary.
            self.has_backspaced = true;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn displayed(&self) -> String {
        if self.disabled {
            return self.chars.iter().collect();
        }
        self.render()
    }

    fn disable_as_literal(&mut self) {
        self.chars = self.render().chars().collect();
        self.tone = 0;
        self.disabled = true;
    }

    fn disable_as_raw(&mut self) {
        self.chars = self.raw.clone();
        self.tone = 0;
        self.disabled = true;
    }

    fn split(&self) -> Split {
        let n = self.chars.len();
        let lc: Vec<char> = self.chars.iter().map(|&c| lower(c)).collect();
        let mut i = 0;
        if n >= 2 && lc[0] == 'q' && lc[1] == 'u' {
            i = 2;
        } else if n >= 2 && lc[0] == 'g' && lc[1] == 'i' {
            i = if n >= 3 && is_base_vowel(lc[2]) { 2 } else { 1 };
        } else {
            while i < n && !is_base_vowel(lc[i]) {
                i += 1;
            }
        }
        let initial: String = lc[..i].iter().collect();
        let mut vidx = Vec::new();
        while i < n && is_base_vowel(lc[i]) {
            vidx.push(i);
            i += 1;
        }
        let vowel: String = vidx.iter().map(|&k| lc[k]).collect();
        let fin: String = lc[i..].iter().collect();
        Split { initial, vowel, fin, vidx }
    }

    fn tone_target(&self, s: &Split) -> Option<usize> {
        let v = &s.vidx;
        let k = v.len();
        if k == 0 {
            return None;
        }
        if k == 1 {
            return Some(v[0]);
        }
        // 'ươ' / 'ưo': the tone goes on 'ơ'.
        for pos in 0..k - 1 {
            let c1 = lower(self.chars[v[pos]]);
            let c2 = lower(self.chars[v[pos + 1]]);
            if c1 == 'ư' && (c2 == 'ơ' || c2 == 'o') {
                return Some(v[pos + 1]);
            }
        }
        // Otherwise the first modified vowel (ă â ê ô ơ ư) takes it.
        for &idx in v {
            if is_modified_vowel(lower(self.chars[idx])) {
                return Some(idx);
            }
        }
        if k == 2 {
            if !s.fin.is_empty() {
                return Some(v[1]); // toán, hoàng
            }
            if matches!(s.vowel.as_str(), "oa" | "oe" | "uy") {
                return Some(if self.cfg.modern_orthography { v[1] } else { v[0] });
            }
            return Some(v[0]);
        }
        Some(v[1]) // oai, oay, uya, khuỷu, ngoèo
    }

    fn render(&self) -> String {
        if self.chars.is_empty() {
            return String::new();
        }
        if self.tone == 0 {
            return self.chars.iter().collect();
        }
        let s = self.split();
        match self.tone_target(&s) {
            None => self.chars.iter().collect(),
            Some(t) => {
                let mut out = self.chars.clone();
                out[t] = tone_form(out[t], self.tone);
                out.into_iter().collect()
            }
        }
    }

    /// Vietnamese phonotactics check of the current syllable.
    fn is_valid(&self, committed: bool) -> bool {
        if self.chars.is_empty() {
            return true;
        }
        if committed {
            let raw: String = self.raw.iter().map(|&c| lower(c)).collect();
            if in_set(ENGLISH_COLLISION_WORDS, &raw) {
                return false;
            }
        }
        let lc: Vec<char> = self.chars.iter().map(|&c| lower(c)).collect();
        if lc.iter().any(|c| matches!(c, 'f' | 'j' | 'w' | 'z')) {
            return false;
        }
        let s = self.split();
        let initials = if committed { VALID_INITIALS } else { VALID_INITIALS_TYPING };
        if !in_set(initials, &s.initial) {
            return false;
        }
        if s.vowel.is_empty() {
            if !s.fin.is_empty() {
                return false;
            }
            // A lone 'đ' (dd) stays Vietnamese at commit, like UniKey.
            return !committed || s.initial == "đ";
        }
        // A second vowel group after the final consonant: 'update', 'service', 'mode'.
        let last_v = *s.vidx.last().unwrap();
        if lc[last_v + 1..].iter().any(|&c| is_base_vowel(c)) {
            return false;
        }
        let first_v = s.vowel.chars().next().unwrap();
        let init = s.initial.as_str();
        if init == "c" && matches!(first_v, 'i' | 'e' | 'ê' | 'y') {
            return false;
        }
        if (init == "gh" || init == "ngh") && !matches!(first_v, 'i' | 'e' | 'ê') {
            return false;
        }
        if init == "k" && !matches!(first_v, 'i' | 'e' | 'ê' | 'y') {
            return false;
        }
        let fin = s.fin.as_str();
        if !fin.is_empty() && !in_set(VALID_FINALS, fin) {
            return false;
        }
        let nuclei = if committed { NUCLEI_COMMITTED } else { NUCLEI_TYPING };
        if !in_set(nuclei, &s.vowel) {
            return false;
        }
        if s.vowel == "oo" {
            let ok = if committed {
                matches!(fin, "c" | "ng")
            } else {
                matches!(fin, "" | "c" | "n" | "ng")
            };
            if !ok {
                return false;
            }
        }
        if in_set(STOP_FINALS, fin) {
            if matches!(self.tone, 2 | 3 | 4) {
                return false;
            }
            if committed && self.tone == 0 {
                return false;
            }
        }
        if committed && !fin.is_empty() {
            if in_set(NO_FINAL_NUCLEI, &s.vowel) {
                return false;
            }
            // Vietnamese spells these 'inh' / 'ich' (things, kings).
            if s.vowel == "i" && (fin == "ng" || fin == "c") {
                return false;
            }
        }
        if committed && !init.is_empty() && fin.is_empty() && in_set(FORBIDDEN_OPEN_NUCLEI, &s.vowel) {
            return false;
        }
        true
    }

    /// Backspace: deletes the last visible character, keeping the tone when it can.
    pub fn backspace(&mut self) -> KeyResult {
        if self.raw.is_empty() && self.chars.is_empty() {
            // Erasing the boundary after a committed word brings that word back for editing.
            if self.prev.is_some() && self.prev_trailing > 0 {
                self.prev_trailing -= 1;
                if self.prev_trailing == 0 {
                    self.restore_prev_word();
                }
            }
            return KeyResult::pass();
        }
        self.has_backspaced = true;
        let old = self.displayed();
        if old.is_empty() {
            self.clear_word();
            return KeyResult::pass();
        }
        if self.disabled {
            self.raw.pop();
            self.chars.pop();
            if self.chars.is_empty() {
                self.clear_word();
            }
            return KeyResult::pass();
        }
        let s = self.split();
        let target = self.tone_target(&s);
        let removed = self.chars.len() - 1;
        self.chars.pop();
        self.raw.pop();
        if self.chars.is_empty() {
            self.clear_word();
            return KeyResult::pass();
        }
        let s2 = self.split();
        if s2.vidx.is_empty() || Some(removed) == target {
            self.tone = 0;
        }
        let new = self.displayed();
        let old_n = old.chars().count();
        let new_n = new.chars().count();
        if old_n == new_n + 1 && old.starts_with(new.as_str()) {
            return KeyResult::pass();
        }
        KeyResult { handled: true, backspaces: old_n, commit: new }
    }

    /// One printable character. Letters (or digits in VNI) compose; anything else ends the word.
    pub fn key(&mut self, ch: char) -> KeyResult {
        let is_vni = self.cfg.input_method == InputMethod::Vni;
        let is_vni_digit = is_vni && ch.is_ascii_digit() && !self.chars.is_empty();
        if !ch.is_alphabetic() && !is_vni_digit {
            return self.boundary(ch);
        }
        let old = self.displayed();
        if is_vni {
            self.process_vni(ch);
        } else {
            self.process_char(ch);
        }
        let new = self.displayed();
        let mut appended = old.clone();
        appended.push(ch);
        if new == appended {
            return KeyResult::pass();
        }
        let o: Vec<char> = old.chars().collect();
        let n: Vec<char> = new.chars().collect();
        let common = o.iter().zip(n.iter()).take_while(|(a, b)| a == b).count();
        KeyResult { handled: true, backspaces: o.len() - common, commit: n[common..].iter().collect() }
    }

    fn find_macro(&self, raw: &str) -> Option<String> {
        if let Some(exp) = self.cfg.macros.get(raw) {
            return Some(exp.clone());
        }
        let lc = raw.to_lowercase();
        if let Some(exp) = self.cfg.macros.get(&lc) {
            if raw.chars().all(|c| c.is_uppercase()) {
                return Some(exp.to_uppercase());
            }
            if raw.chars().next().map_or(false, |c| c.is_uppercase()) {
                let mut chars = exp.chars();
                if let Some(first) = chars.next() {
                    return Some(first.to_uppercase().collect::<String>() + chars.as_str());
                }
            }
            return Some(exp.clone());
        }
        None
    }

    fn boundary(&mut self, ch: char) -> KeyResult {
        if self.raw.is_empty() && self.chars.is_empty() {
            if self.prev.is_some() {
                self.prev_trailing += 1;
            }
            return KeyResult::pass();
        }
        let old = self.displayed();
        let raw: String = self.raw.iter().collect();

        // 1. Gõ tắt (Macro)
        if self.cfg.enable_macro && !self.has_backspaced && !raw.is_empty() {
            if let Some(expanded) = self.find_macro(&raw) {
                self.commit_word();
                let mut commit = expanded;
                commit.push(ch);
                return KeyResult {
                    handled: true,
                    backspaces: old.chars().count(),
                    commit,
                };
            }
        }

        // 2. Tự động trả lại từ tiếng Anh / từ trong custom_words
        if self.cfg.auto_restore_english && !self.has_backspaced && !raw.is_empty() && old != raw {
            let raw_lc = raw.to_lowercase();
            let is_custom = self.cfg.custom_words.iter().any(|w| w.to_lowercase() == raw_lc);
            let restore = is_custom || if self.disabled {
                in_set(ENGLISH_COLLISION_WORDS, &raw_lc)
            } else {
                !self.is_valid(true)
            };
            if restore {
                self.disable_as_raw();
                self.commit_word();
                let mut commit = raw;
                commit.push(ch);
                return KeyResult { handled: true, backspaces: old.chars().count(), commit };
            }
        }
        self.commit_word();
        KeyResult::pass()
    }

    fn process_vni(&mut self, ch: char) {
        self.raw.push(ch);
        if self.disabled {
            self.chars.push(ch);
            return;
        }
        let restore = self.cfg.auto_restore_english;

        // Dấu đ: 9
        if ch == '9' && !self.chars.is_empty() {
            let c0 = self.chars[0];
            let l0 = lower(c0);
            if l0 == 'd' {
                self.chars[0] = cased('đ', c0);
                self.last_mod = Some('9');
                return;
            } else if l0 == 'đ' && self.last_mod == Some('9') {
                self.chars[0] = cased('d', c0);
                self.chars.push(ch);
                self.disable_as_literal();
                return;
            }
        }

        // Dấu thanh: 1..=5, 0 xóa dấu
        if matches!(ch, '0'..='5') {
            let s = self.split();
            if !s.vidx.is_empty() {
                let new_tone = match ch {
                    '1' => 1,
                    '2' => 2,
                    '3' => 3,
                    '4' => 4,
                    '5' => 5,
                    _ => 0,
                };
                if new_tone == 0 {
                    if self.tone != 0 {
                        self.tone = 0;
                        self.last_mod = Some('0');
                        return;
                    }
                } else {
                    if self.tone == new_tone && self.last_mod == Some(ch) {
                        self.tone = 0;
                        self.chars.push(ch);
                        self.disable_as_literal();
                        return;
                    }
                    self.tone = new_tone;
                    self.last_mod = Some(ch);
                    if restore && !self.is_valid(false) {
                        self.disable_as_raw();
                    }
                    return;
                }
            }
        }

        let s = self.split();

        // 6: thêm mũ â, ê, ô (và escape 66 -> 6)
        if ch == '6' {
            if self.last_mod == Some('6') {
                for &idx in s.vidx.iter().rev() {
                    if let Some(plain) = unhat(self.chars[idx]) {
                        self.chars[idx] = plain;
                    }
                }
                self.chars.push(ch);
                self.disable_as_literal();
                self.last_mod = None;
                return;
            }
            for &idx in s.vidx.iter().rev() {
                let cur = self.chars[idx];
                if matches!(lower(cur), 'a' | 'e' | 'o') {
                    if let Some(h) = hat(cur) {
                        self.chars[idx] = h;
                        if in_set(CIRCUMFLEX_NUCLEI, &self.split().vowel) {
                            self.last_mod = Some('6');
                            if restore && !self.is_valid(false) {
                                self.disable_as_raw();
                            }
                            return;
                        }
                        self.chars[idx] = cur;
                    }
                }
            }
        }

        // 7: thêm móc/râu ơ, ư, ươ (và escape 77 -> 7)
        if ch == '7' {
            if self.last_mod == Some('7') {
                for &idx in s.vidx.iter().rev() {
                    if let Some(p) = unw(self.chars[idx]) {
                        self.chars[idx] = p;
                    }
                }
                self.chars.push(ch);
                self.disable_as_literal();
                self.last_mod = None;
                return;
            }
            if !s.vidx.is_empty() {
                for pos in 0..s.vidx.len().saturating_sub(1) {
                    let (i1, i2) = (s.vidx[pos], s.vidx[pos + 1]);
                    let (v1, v2) = (lower(self.chars[i1]), lower(self.chars[i2]));
                    if matches!(v1, 'u' | 'ư') && matches!(v2, 'o' | 'ô' | 'ơ') {
                        self.chars[i1] = cased('ư', self.chars[i1]);
                        self.chars[i2] = cased('ơ', self.chars[i2]);
                        self.last_mod = Some('7');
                        return;
                    }
                }
                if s.vowel == "ua" && s.initial != "qu" {
                    let i1 = s.vidx[0];
                    self.chars[i1] = cased('ư', self.chars[i1]);
                    self.last_mod = Some('7');
                    return;
                }
                for (p, &idx) in s.vidx.iter().enumerate().rev() {
                    let vc = self.chars[idx];
                    let vl = lower(vc);
                    if matches!(vl, 'o' | 'ô' | 'ơ') {
                        self.chars[idx] = cased('ơ', vc);
                        self.last_mod = Some('7');
                        if p > 0 {
                            let prev_idx = s.vidx[p - 1];
                            if lower(self.chars[prev_idx]) == 'u' && !s.fin.is_empty() {
                                self.chars[prev_idx] = cased('ư', self.chars[prev_idx]);
                            }
                        }
                        if restore && !self.is_valid(false) {
                            self.disable_as_raw();
                        }
                        return;
                    }
                    if matches!(vl, 'u' | 'ư') {
                        self.chars[idx] = cased('ư', vc);
                        self.last_mod = Some('7');
                        if restore && !self.is_valid(false) {
                            self.disable_as_raw();
                        }
                        return;
                    }
                }
            }
        }

        // 8: thêm trăng ă (và escape 88 -> 8)
        if ch == '8' {
            if self.last_mod == Some('8') {
                for &idx in s.vidx.iter().rev() {
                    if let Some(p) = unw(self.chars[idx]) {
                        self.chars[idx] = p;
                    }
                }
                self.chars.push(ch);
                self.disable_as_literal();
                self.last_mod = None;
                return;
            }
            for &idx in s.vidx.iter().rev() {
                let vc = self.chars[idx];
                let vl = lower(vc);
                if matches!(vl, 'a' | 'ă' | 'â') {
                    self.chars[idx] = cased('ă', vc);
                    self.last_mod = Some('8');
                    if restore && !self.is_valid(false) {
                        self.disable_as_raw();
                    }
                    return;
                }
            }
        }

        self.chars.push(ch);
        self.last_mod = None;

        if restore && !self.is_valid(false) {
            self.disable_as_raw();
        }
    }

    fn process_char(&mut self, ch: char) {
        self.raw.push(ch);
        if self.disabled {
            self.chars.push(ch);
            return;
        }
        let cl = lower(ch);
        let restore = self.cfg.auto_restore_english;

        // dd -> đ (and ddd -> dd)
        if cl == 'd' && !self.chars.is_empty() {
            let c0 = self.chars[0];
            let l0 = lower(c0);
            if (l0 == 'd' || l0 == 'đ') && (self.chars.len() == 1 || self.last_mod != Some('d')) {
                if l0 == 'd' {
                    self.chars[0] = cased('đ', c0);
                    self.last_mod = Some('d');
                    return;
                } else if self.last_mod == Some(cl) {
                    self.chars[0] = cased('d', c0);
                    self.chars.push(ch);
                    self.disable_as_literal();
                    return;
                }
            }
        }

        // Tones: s f r x j, z removes.
        if let Some(new_tone) = tone_key(cl) {
            let s = self.split();
            if !s.vidx.is_empty() {
                if new_tone == 0 {
                    if self.tone != 0 {
                        self.tone = 0;
                        self.last_mod = Some(cl);
                        return;
                    }
                } else {
                    if self.tone == new_tone && self.last_mod == Some(cl) {
                        self.tone = 0;
                        self.chars.push(ch);
                        self.disable_as_literal();
                        return;
                    }
                    self.tone = new_tone;
                    self.last_mod = Some(cl);
                    if restore && !self.is_valid(false) {
                        self.disable_as_raw();
                    }
                    return;
                }
            }
        }

        let s = self.split();

        // aa -> â, ee -> ê, oo -> ô (and the triple-key escape).
        if matches!(cl, 'a' | 'e' | 'o') {
            for &idx in s.vidx.iter().rev() {
                if let Some(plain) = unhat(self.chars[idx]) {
                    if self.last_mod == Some(cl) {
                        self.chars[idx] = plain;
                        self.chars.push(ch);
                        // 'xoọc', 'boong' really have a double 'oo'.
                        if self.split().vowel != "oo" {
                            self.disable_as_literal();
                        } else {
                            self.raw.pop();
                        }
                        self.last_mod = None;
                        return;
                    }
                }
            }
            for &idx in s.vidx.iter().rev() {
                let cur = self.chars[idx];
                if lower(cur) == cl {
                    if let Some(h) = hat(cur) {
                        self.chars[idx] = h;
                        if in_set(CIRCUMFLEX_NUCLEI, &self.split().vowel) {
                            self.last_mod = Some(cl);
                            if restore && !self.is_valid(false) {
                                self.disable_as_raw();
                            }
                            return;
                        }
                        // e.g. 'oe' + 'o' is 'oeo' (ngoèo), not 'ôe'.
                        self.chars[idx] = cur;
                    }
                }
            }
        }

        // w: ă ơ ư ươ, standalone ư, and ww -> w.
        if cl == 'w' {
            if self.last_mod == Some('w') {
                let last = self.chars.len().wrapping_sub(1);
                let n = self.raw.len();
                let standalone = s.vidx.len() == 1
                    && s.vidx[0] == last
                    && matches!(self.chars[last], 'ư' | 'Ư')
                    && n >= 2
                    && lower(self.raw[n - 2]) == 'w'
                    && !(n >= 3 && lower(self.raw[n - 3]) == 'u');
                if standalone {
                    self.chars[last] = self.raw[n - 2];
                } else {
                    for &idx in s.vidx.iter().rev() {
                        if let Some(p) = unw(self.chars[idx]) {
                            self.chars[idx] = p;
                        }
                    }
                    self.chars.push(ch);
                }
                self.disable_as_literal();
                return;
            }
            if !s.vidx.is_empty() {
                // uo / uô / ưo + w -> ươ
                for pos in 0..s.vidx.len() - 1 {
                    let (i1, i2) = (s.vidx[pos], s.vidx[pos + 1]);
                    let (v1, v2) = (lower(self.chars[i1]), lower(self.chars[i2]));
                    if matches!(v1, 'u' | 'ư') && matches!(v2, 'o' | 'ô' | 'ơ') {
                        self.chars[i1] = cased('ư', self.chars[i1]);
                        self.chars[i2] = cased('ơ', self.chars[i2]);
                        self.last_mod = Some('w');
                        return;
                    }
                }
                // ua + w -> ưa (mưa, chưa), but not after 'qu'.
                if s.vowel == "ua" && s.initial != "qu" {
                    let i1 = s.vidx[0];
                    self.chars[i1] = cased('ư', self.chars[i1]);
                    self.last_mod = Some('w');
                    return;
                }
                for (p, &idx) in s.vidx.iter().enumerate().rev() {
                    let vc = self.chars[idx];
                    let vl = lower(vc);
                    if matches!(vl, 'a' | 'ă' | 'â') {
                        self.chars[idx] = cased('ă', vc);
                        self.last_mod = Some('w');
                        if restore && !self.is_valid(false) {
                            self.disable_as_raw();
                        }
                        return;
                    }
                    if matches!(vl, 'o' | 'ô' | 'ơ') {
                        self.chars[idx] = cased('ơ', vc);
                        self.last_mod = Some('w');
                        if p > 0 {
                            let prev_idx = s.vidx[p - 1];
                            if lower(self.chars[prev_idx]) == 'u' && !s.fin.is_empty() {
                                self.chars[prev_idx] = cased('ư', self.chars[prev_idx]);
                            }
                        }
                        if restore && !self.is_valid(false) {
                            self.disable_as_raw();
                        }
                        return;
                    }
                    if matches!(vl, 'u' | 'ư') {
                        self.chars[idx] = cased('ư', vc);
                        self.last_mod = Some('w');
                        if restore && !self.is_valid(false) {
                            self.disable_as_raw();
                        }
                        return;
                    }
                }
            }
            if s.vidx.is_empty() {
                let init: String = self.chars.iter().map(|&c| lower(c)).collect();
                if in_set(VALID_INITIALS, &init) && init != "q" {
                    self.chars.push(cased('ư', ch));
                    self.last_mod = Some('w');
                    return;
                }
            }
        }

        self.chars.push(ch);
        self.last_mod = None;

        // uơ / ưo + final or glide -> ươ
        let s = self.split();
        if (matches!(s.vowel.as_str(), "uơ" | "ưo") && !s.fin.is_empty())
            || matches!(s.vowel.as_str(), "uơi" | "ưoi" | "uơu" | "ưou")
        {
            let (i1, i2) = (s.vidx[0], s.vidx[1]);
            self.chars[i1] = cased('ư', self.chars[i1]);
            self.chars[i2] = cased('ơ', self.chars[i2]);
        }

        if restore && !self.is_valid(false) {
            self.disable_as_raw();
        }
    }

    /// Simulates an app text field: feeds keys ('\u{8}' is Backspace) and returns the text.
    pub fn feed(&mut self, text: &str) -> String {
        self.reset();
        let mut screen: Vec<char> = Vec::new();
        for ch in text.chars() {
            let r = if ch == '\u{8}' { self.backspace() } else { self.key(ch) };
            if r.handled {
                for _ in 0..r.backspaces {
                    screen.pop();
                }
                screen.extend(r.commit.chars());
            } else if ch == '\u{8}' {
                screen.pop();
            } else {
                screen.push(ch);
            }
        }
        screen.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(s: &str) -> String {
        Telex::new(Config::default()).feed(s)
    }

    #[test]
    fn words() {
        let cases = [
            ("Kieemr tra mays tooi hieenj taij", "Kiểm tra máy tôi hiện tại"),
            ("toanf booj gox vaof terminal cuar banj ddang bij looix", "toàn bộ gõ vào terminal của bạn đang bị lỗi"),
            ("vieetj nam", "việt nam"),
            ("nguowif", "người"),
            ("dduwowcj", "được"),
            ("hoaf", "hòa"),
            ("khoer", "khỏe"),
            ("thuys", "thúy"),
            ("quas", "quá"),
            ("gias", "giá"),
            ("giuwowngf", "giường"),
            ("muwa muaw", "mưa mưa"),
            ("cuwar", "cửa"),
            ("trawng", "trăng"),
            ("khuyeens", "khuyến"),
            ("ruwowuj", "rượu"),
            ("hoawcj", "hoặc"),
            ("khoong", "không"),
            ("VIEETJ NAM", "VIỆT NAM"),
            ("w", "ư"),
            ("ww", "w"),
            ("aa aaa", "â aa"),
            ("ddi dddi", "đi ddi"),
        ];
        for (input, want) in cases {
            assert_eq!(feed(input), want, "input {input:?}");
        }
    }

    #[test]
    fn english_restore() {
        for w in ["window", "software", "book", "issue", "pass", "user", "docker", "case", "fix", "for", "terminal"] {
            assert_eq!(feed(&format!("{w} ")), format!("{w} "), "word {w}");
        }
    }

    #[test]
    fn modern_orthography() {
        let mut t = Telex::new(Config {
            modern_orthography: true,
            auto_restore_english: true,
            ..Default::default()
        });
        assert_eq!(t.feed("hoaf khoer thuys"), "hoà khoẻ thuý");
    }

    /// Every case in tests/golden.tsv; regenerate with `python3 -I tools/golden.py update`.
    #[test]
    fn golden_cases() {
        let mut t = Telex::new(Config::default());
        let (mut total, mut changed) = (0, Vec::new());
        for line in include_str!("../tests/golden.tsv").lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            assert!(cols.len() == 3 && cols[2] == ";", "bad golden line: {line:?}");
            let unesc = |s: &str| s.replace("\\b", "\u{8}").replace("\\\\", "\\");
            let (input, want) = (unesc(cols[0]), unesc(cols[1]));
            total += 1;
            let got = t.feed(&input);
            if got != want {
                changed.push(format!("{input:?}: want {want:?}, got {got:?}"));
            }
        }
        assert!(total > 10_000, "golden set too small: {total}");
        assert!(changed.is_empty(), "{} of {total} golden cases changed:\n{}", changed.len(), changed[..changed.len().min(20)].join("\n"));
    }

    #[test]
    fn backspace_keeps_tone() {
        assert_eq!(feed("hoafn\u{8}"), "hòa");
        assert_eq!(feed("tieengs\u{8}"), "tiến");
    }

    #[test]
    fn vni_typing() {
        let mut t = Telex::new(Config {
            input_method: InputMethod::Vni,
            ..Default::default()
        });
        let cases = [
            ("Vie6t5 Nam d9e5p", "Việt Nam đẹp"),
            ("toan1", "toán"),
            ("d9i", "đi"),
            ("mu7a", "mưa"),
            ("d9u7o7ng2", "đường"),
            ("kho6ng", "không"),
            ("d99", "d9"),
            ("a66", "a6"),
            ("toan11", "toan1"),
            ("123 456", "123 456"),
        ];
        for (input, want) in cases {
            assert_eq!(t.feed(input), want, "VNI input {input:?}");
        }
    }

    #[test]
    fn macro_expansion() {
        let mut macros = HashMap::new();
        macros.insert("vn".into(), "Việt Nam".into());
        macros.insert("hn".into(), "Hà Nội".into());
        let mut t = Telex::new(Config {
            enable_macro: true,
            macros,
            ..Default::default()
        });
        assert_eq!(t.feed("vn "), "Việt Nam ");
        assert_eq!(t.feed("VN "), "VIỆT NAM ");
        assert_eq!(t.feed("Vn "), "Việt Nam ");
        assert_eq!(t.feed("hn."), "Hà Nội.");
    }

    #[test]
    fn custom_english_words() {
        let mut t = Telex::new(Config {
            custom_words: vec!["this".into(), "test".into()],
            ..Default::default()
        });
        assert_eq!(t.feed("this "), "this ");
        assert_eq!(t.feed("This "), "This ");
        assert_eq!(t.feed("test "), "test ");
    }
}

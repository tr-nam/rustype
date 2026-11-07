//! IBus-facing behaviour of one engine instance: picks how to show text in each app,
//! and turns key events into IBus actions.
//!
//! Three ways to put a word on screen:
//! - Surrounding: the app reports Surrounding Text (most GUI apps). Edits are
//!   DeleteSurroundingText + CommitText, with no underline.
//! - Preedit: the app has no Surrounding Text (terminals such as Ptyxis/VTE). The word
//!   is shown underlined and committed at the word boundary. It is sent in COMMIT mode,
//!   so GNOME/Mutter commits it instead of dropping it on a mouse click or focus change.
//! - Backspace: fake BackSpace key events. Only used when the config forces `direct`.

use crate::config::{Config, Mode};
use rustype_core::telex::Telex;

pub const CAP_SURROUNDING_TEXT: u32 = 1 << 5;
const PURPOSE_PASSWORD: u32 = 8;
const PURPOSE_PIN: u32 = 9;

const CONTROL_MASK: u32 = 1 << 2;
const MOD1_MASK: u32 = 1 << 3; // Alt
const MOD4_MASK: u32 = 1 << 6; // Super on most layouts
const SUPER_MASK: u32 = 1 << 26;
const HYPER_MASK: u32 = 1 << 27;
const META_MASK: u32 = 1 << 28;
pub const RELEASE_MASK: u32 = 1 << 30;

pub const KEY_BACKSPACE: u32 = 0xff08;

/// Something to send to IBus, in order, before the ProcessKeyEvent reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    Commit(String),
    Preedit(String),
    HidePreedit,
    DeleteSurrounding { offset: i32, nchars: u32 },
    ForwardKey { keyval: u32, keycode: u32, state: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Show {
    Surrounding,
    Preedit,
    Backspace,
}

pub struct Ime {
    telex: Telex,
    mode: Mode,
    caps: u32,
    purpose: u32,
    show: Show,
    preedit_visible: bool,
}

fn is_modifier_key(keyval: u32) -> bool {
    // Shift_L..Hyper_R, ISO_Level3_Shift and friends, Mode_switch.
    (0xffe1..=0xffee).contains(&keyval) || (0xfe01..=0xfe0f).contains(&keyval) || keyval == 0xff7e
}

impl Ime {
    pub fn new(cfg: &Config) -> Self {
        Ime {
            telex: Telex::new(cfg.telex.clone()),
            mode: cfg.mode,
            caps: 0,
            purpose: 0,
            show: Show::Preedit,
            preedit_visible: false,
        }
    }

    pub fn apply_config(&mut self, cfg: &Config) {
        if self.telex.is_empty() && !self.preedit_visible {
            self.telex.cfg = cfg.telex.clone();
            self.mode = cfg.mode;
        }
    }

    /// Fresh state after an internal error, keeping what the app told us.
    pub fn recover(&mut self, cfg: &Config) {
        let (caps, purpose) = (self.caps, self.purpose);
        *self = Ime::new(cfg);
        self.caps = caps;
        self.purpose = purpose;
    }

    pub fn set_capabilities(&mut self, caps: u32) {
        self.caps = caps;
    }

    pub fn set_content_type(&mut self, purpose: u32, _hints: u32) {
        self.purpose = purpose;
    }

    /// The display method is chosen when a word starts and kept until it ends.
    fn pick_show(&mut self) -> Show {
        if self.telex.is_empty() && !self.preedit_visible {
            let surrounding = self.caps & CAP_SURROUNDING_TEXT != 0;
            self.show = match self.mode {
                Mode::Preedit => Show::Preedit,
                Mode::Auto if surrounding => Show::Surrounding,
                Mode::Auto => Show::Preedit,
                Mode::Direct if surrounding => Show::Surrounding,
                Mode::Direct => Show::Backspace,
            };
        }
        self.show
    }

    fn replace(&self, backspaces: usize, text: String, out: &mut Vec<Out>) {
        if backspaces > 0 {
            match self.show {
                Show::Backspace => {
                    for _ in 0..backspaces {
                        out.push(Out::ForwardKey { keyval: KEY_BACKSPACE, keycode: 14, state: 0 });
                        out.push(Out::ForwardKey { keyval: KEY_BACKSPACE, keycode: 14, state: RELEASE_MASK });
                    }
                }
                _ => out.push(Out::DeleteSurrounding { offset: -(backspaces as i32), nchars: backspaces as u32 }),
            }
        }
        if !text.is_empty() {
            out.push(Out::Commit(text));
        }
    }

    fn show_preedit(&mut self, out: &mut Vec<Out>) {
        let word = self.telex.displayed();
        if word.is_empty() {
            self.hide_preedit(out);
        } else {
            out.push(Out::Preedit(word));
            self.preedit_visible = true;
        }
    }

    fn hide_preedit(&mut self, out: &mut Vec<Out>) {
        if self.preedit_visible {
            out.push(Out::HidePreedit);
            self.preedit_visible = false;
        }
    }

    /// Our own commit of the preedit (Enter, shortcuts...). Hide first so the client never
    /// holds a stale COMMIT-mode copy that a later reset would commit a second time.
    fn flush_preedit(&mut self, out: &mut Vec<Out>) {
        if self.preedit_visible {
            let word = self.telex.displayed();
            self.hide_preedit(out);
            if !word.is_empty() {
                out.push(Out::Commit(word));
            }
        }
    }

    /// Reset / focus out. The client already committed a COMMIT-mode preedit itself,
    /// so only the engine state is dropped (committing it here would type it twice).
    pub fn reset(&mut self, out: &mut Vec<Out>) {
        self.hide_preedit(out);
        self.telex.reset();
    }

    pub fn process_key(&mut self, keyval: u32, _keycode: u32, state: u32, out: &mut Vec<Out>) -> bool {
        if state & RELEASE_MASK != 0 || is_modifier_key(keyval) {
            return false;
        }
        let shortcut = CONTROL_MASK | MOD1_MASK | MOD4_MASK | SUPER_MASK | HYPER_MASK | META_MASK;
        if state & shortcut != 0 || self.purpose == PURPOSE_PASSWORD || self.purpose == PURPOSE_PIN {
            self.flush_preedit(out);
            self.telex.reset();
            return false;
        }

        let show = self.pick_show();
        if keyval == KEY_BACKSPACE {
            return self.backspace(show, out);
        }
        if (0x20..=0x7e).contains(&keyval) {
            let ch = char::from_u32(keyval).unwrap();
            return match show {
                Show::Preedit => self.printable_preedit(ch, out),
                _ => {
                    let r = self.telex.key(ch);
                    if !r.handled {
                        return false;
                    }
                    self.replace(r.backspaces, r.commit, out);
                    true
                }
            };
        }
        // Enter, Tab, Escape, arrows, F-keys, keypad...: end the word and let the app have the key.
        self.flush_preedit(out);
        self.telex.reset();
        false
    }

    fn backspace(&mut self, show: Show, out: &mut Vec<Out>) -> bool {
        if show != Show::Preedit {
            let r = self.telex.backspace();
            if !r.handled {
                return false;
            }
            self.replace(r.backspaces, r.commit, out);
            return true;
        }
        if !self.preedit_visible {
            // The word before the cursor is already committed: let the app erase it.
            self.telex.reset();
            return false;
        }
        self.telex.backspace();
        self.show_preedit(out);
        if !self.preedit_visible {
            // The last preedit char is gone; the app holds nothing of it, so swallow the key.
            self.telex.reset();
        }
        true
    }

    fn printable_preedit(&mut self, ch: char, out: &mut Vec<Out>) -> bool {
        let before = self.telex.displayed();
        let r = self.telex.key(ch);
        if ch.is_alphabetic() {
            self.show_preedit(out);
            return true;
        }
        // Word boundary: commit the word and the boundary char together, in one CommitText.
        self.hide_preedit(out);
        // The previous word cannot be edited back from preedit mode.
        self.telex.reset();
        let mut text = if r.handled { r.commit } else { before };
        if !r.handled {
            if text.is_empty() {
                return false;
            }
            text.push(ch);
        }
        out.push(Out::Commit(text));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_RETURN: u32 = 0xff0d;

    fn keys(ime: &mut Ime, s: &str) -> Vec<Out> {
        let mut out = Vec::new();
        for ch in s.chars() {
            let kv = if ch == '\u{8}' { KEY_BACKSPACE } else if ch == '\n' { KEY_RETURN } else { ch as u32 };
            ime.process_key(kv, 0, 0, &mut out);
        }
        out
    }

    #[test]
    fn terminal_uses_commit_mode_preedit() {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9); // PREEDIT_TEXT | FOCUS, no surrounding text (VTE)
        let out = keys(&mut ime, "kieemr ");
        assert!(out.contains(&Out::Preedit("kiểm".into())));
        assert_eq!(out.last(), Some(&Out::Commit("kiểm ".into())));
        assert!(!out.iter().any(|o| matches!(o, Out::DeleteSurrounding { .. } | Out::ForwardKey { .. })));
    }

    #[test]
    fn reset_does_not_commit_again() {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9);
        keys(&mut ime, "kieemr");
        let mut out = Vec::new();
        ime.reset(&mut out);
        assert_eq!(out, vec![Out::HidePreedit]);
    }

    #[test]
    fn gui_app_uses_surrounding_text() {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9 | CAP_SURROUNDING_TEXT);
        let mut text = String::new();
        for ch in "kieemr tra ".chars() {
            let mut out = Vec::new();
            if !ime.process_key(ch as u32, 0, 0, &mut out) {
                text.push(ch);
            }
            for o in out {
                match o {
                    Out::DeleteSurrounding { nchars, .. } => {
                        for _ in 0..nchars {
                            text.pop();
                        }
                    }
                    Out::Commit(s) => text.push_str(&s),
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        assert_eq!(text, "kiểm tra ");
    }

    #[test]
    fn enter_commits_then_passes() {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9);
        let mut out = Vec::new();
        for ch in "vieetj".chars() {
            ime.process_key(ch as u32, 0, 0, &mut out);
        }
        out.clear();
        let handled = ime.process_key(KEY_RETURN, 0, 0, &mut out);
        assert!(!handled);
        assert_eq!(out, vec![Out::HidePreedit, Out::Commit("việt".into())]);
    }

    #[test]
    fn password_fields_bypass() {
        let mut ime = Ime::new(&Config::default());
        ime.set_content_type(PURPOSE_PASSWORD, 0);
        let mut out = Vec::new();
        assert!(!ime.process_key('a' as u32, 0, 0, &mut out));
        assert!(out.is_empty());
    }
}

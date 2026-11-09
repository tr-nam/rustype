//! IBus-facing behaviour of one engine instance: picks how to show text in each app,
//! and turns key events into IBus actions.
//!
//! Two ways to put a word on screen:
//! - Surrounding: the app reports Surrounding Text (most GUI apps). Edits are
//!   DeleteSurroundingText + CommitText, with no underline.
//! - Preedit: the app has no Surrounding Text (terminals such as Ptyxis/VTE). The word
//!   is shown underlined and committed at the word boundary. It is sent in COMMIT mode,
//!   so GNOME/Mutter commits it instead of dropping it on a mouse click or focus change.
//!   `direct` without Surrounding Text also uses this way.

use crate::config::{Config, Mode, ToggleShortcut};
use rustype_core::telex::Telex;

pub const CAP_SURROUNDING_TEXT: u32 = 1 << 5;
const PURPOSE_PASSWORD: u32 = 8;
const PURPOSE_PIN: u32 = 9;
const PURPOSE_URL: u32 = 5;

const CONTROL_MASK: u32 = 1 << 2;
const MOD1_MASK: u32 = 1 << 3; // Alt
const MOD4_MASK: u32 = 1 << 6; // Super on most layouts
const SUPER_MASK: u32 = 1 << 26;
const HYPER_MASK: u32 = 1 << 27;
const META_MASK: u32 = 1 << 28;
pub(crate) const RELEASE_MASK: u32 = 1 << 30;
/// Modifiers other than Ctrl and Shift: a chord with any of them held is not the toggle.
const OTHER_MODS: u32 = MOD1_MASK | MOD4_MASK | SUPER_MASK | HYPER_MASK | META_MASK;

pub(crate) const KEY_BACKSPACE: u32 = 0xff08;

/// Something to send to IBus, in order, before the ProcessKeyEvent reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    Commit(String),
    Preedit(String),
    HidePreedit,
    DeleteSurrounding {
        offset: i32,
        nchars: u32,
    },
    /// Ask the client to report Surrounding Text (IBus RequireSurroundingText).
    RequireSurrounding,
    /// The Ctrl+Shift chord was completed: the service flips Vietnamese on or off, as the panel
    /// menu does.
    ToggleVietnamese,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Show {
    Surrounding,
    Preedit,
}

/// Ctrl+Shift toggle tracking. Fires once per chord, when one of the two keys is released after
/// both were down with no other key or modifier in between.
#[derive(Default)]
struct Chord {
    /// Ctrl and Shift keys currently down (CHORD_CTRL | CHORD_SHIFT bits).
    held: u8,
    /// Alt, Super, Meta or Hyper key currently down.
    other_down: bool,
    state: ChordState,
}

const CHORD_CTRL: u8 = 1;
const CHORD_SHIFT: u8 = 2;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum ChordState {
    /// No chord in progress.
    #[default]
    Idle,
    /// The first of the two keys went down and nothing else has since.
    Armed,
    /// Cancelled, or already fired: nothing fires until every chord key is up again.
    Spent,
}

/// Alt, Meta, Super and Hyper, left and right: keys whose presence cancels the Ctrl+Shift chord.
fn is_other_mod_key(keyval: u32) -> bool {
    (0xffe7..=0xffea).contains(&keyval) || (0xffeb..=0xffee).contains(&keyval)
}

impl Chord {
    /// Feeds one key event. True when this event completes the toggle.
    fn observe(&mut self, keyval: u32, state: u32) -> bool {
        let released = state & RELEASE_MASK != 0;
        let bit = match keyval {
            0xffe3 | 0xffe4 => CHORD_CTRL,
            0xffe1 | 0xffe2 => CHORD_SHIFT,
            _ => 0,
        };
        let other_mod = state & OTHER_MODS != 0;
        if is_other_mod_key(keyval) {
            if released {
                self.other_down = false;
            } else {
                self.other_down = true;
                if self.held != 0 {
                    self.state = ChordState::Spent;
                }
            }
            return false;
        }
        if bit == 0 {
            // Any other key cancels a chord that has a key down.
            if self.held != 0 && (!released || other_mod) {
                self.state = ChordState::Spent;
            }
            return false;
        }
        if released {
            let fire = !other_mod
                && self.state == ChordState::Armed
                && self.held == (CHORD_CTRL | CHORD_SHIFT);
            if fire {
                self.state = ChordState::Spent;
            }
            self.held &= !bit;
            if self.held == 0 {
                self.state = ChordState::Idle;
            }
            return fire;
        }
        if self.held & bit != 0 {
            // Key repeat of a chord key that is already down.
            return false;
        }
        if self.held == 0 {
            self.state = if other_mod || self.other_down {
                ChordState::Spent
            } else {
                ChordState::Armed
            };
        } else if other_mod {
            self.state = ChordState::Spent;
        }
        self.held |= bit;
        false
    }
}

pub struct Ime {
    telex: Telex,
    chord: Chord,
    /// Whether the config enables the Ctrl+Shift toggle.
    toggle_enabled: bool,
    mode: Mode,
    caps: u32,
    purpose: u32,
    show: Show,
    preedit_visible: bool,
    app_id: Option<String>,
    has_selection: bool,
    enabled: bool,
    /// Config that arrived while a word was composing; applied once the word ends.
    pending_cfg: Option<Config>,
    /// Surrounding Text was requested in this focus (URL fields ask once).
    surrounding_requested: bool,
    /// What the URL field's tail may hold, see `FieldModel`.
    model: FieldModel,
}

/// What a URL field's tail may really hold. Chrome's Surrounding Text reports lag behind the keys,
/// and a delete is dropped when a suggestion is selected, so the engine keeps every text the tail
/// may be: `want` (the word as typed) first, then the variants where a delete was dropped.
/// Nothing is tracked while `candidates` is empty.
#[derive(Default)]
struct FieldModel {
    want: String,
    candidates: Vec<String>,
    /// The boundary char that ended the previous word, see `settle`.
    carried: Option<char>,
}

// Lagging reports keep many variants alive while suggestions drop deletes. At 16 candidates the
// model was cleared mid-word and the stale text stayed in the field (tests/omnibox_model.rs).
const MODEL_MAX_CANDIDATES: usize = 256;
const MODEL_MAX_CHARS: usize = 256;

/// The text after the second-last separator `p`: the current word and its boundary. Variants can
/// carry more words than the window (a dropped delete merges them), so one removal per boundary
/// is not enough. None when the variant has no separator at all.
fn last_word(c: &str, p: char) -> Option<String> {
    let at: Vec<usize> = c.match_indices(p).map(|(i, _)| i).collect();
    match at.len() {
        0 => None,
        1 => Some(c.to_string()),
        n => Some(c[at[n - 2] + p.len_utf8()..].to_string()),
    }
}

impl FieldModel {
    fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    fn clear(&mut self) {
        self.want.clear();
        self.candidates.clear();
        self.carried = None;
    }

    /// A word boundary with the word typed. Only the last two words stay tracked: at the next
    /// boundary the first word is forgotten, so a late report can still repair the word before
    /// the current one, but the variants never pile up across a whole sentence.
    fn settle(&mut self, boundary: char) {
        if self.candidates.len() <= 1 {
            self.clear();
            return;
        }
        if let Some(p) = self.carried {
            let mut next = Vec::new();
            for c in &self.candidates {
                match last_word(c, p) {
                    Some(w) => next.push(w),
                    None => {
                        self.clear();
                        return;
                    }
                }
            }
            self.store(next);
        }
        self.carried = if self.is_empty() {
            None
        } else {
            Some(boundary)
        };
    }

    /// Keeps the candidates without duplicates, `want` first. Clears the model when a limit is
    /// exceeded or when no tail is left to track.
    fn store(&mut self, list: Vec<String>) {
        let mut kept: Vec<String> = Vec::new();
        for c in list {
            if c.chars().count() > MODEL_MAX_CHARS {
                self.clear();
                return;
            }
            if !kept.contains(&c) {
                kept.push(c);
            }
        }
        let tracked = kept.first().is_some_and(|w| !w.is_empty());
        if kept.len() > MODEL_MAX_CANDIDATES || !tracked {
            self.clear();
            return;
        }
        self.want = kept[0].clone();
        self.candidates = kept;
    }

    /// A char was typed without the engine handling it: the tail gains it.
    fn native(&mut self, ch: char) {
        let base: Vec<String> = if self.is_empty() {
            vec![String::new()]
        } else {
            self.candidates.clone()
        };
        let next: Vec<String> = base
            .into_iter()
            .map(|mut c| {
                c.push(ch);
                c
            })
            .collect();
        self.store(next);
    }

    /// The engine replaces the last `n` chars of the tail with `commit`. Chrome may have applied
    /// that, or dropped the delete and kept the stale tail under the commit.
    fn edit(&mut self, n: usize, commit: &str) {
        let base: Vec<String> = if self.is_empty() {
            if n > 0 {
                return; // the tail is unknown, so an edit into it cannot be modelled
            }
            vec![String::new()]
        } else {
            self.candidates.clone()
        };
        let mut next: Vec<String> = Vec::new();
        for c in &base {
            let len = c.chars().count();
            if len < n {
                self.clear();
                return;
            }
            let keep: String = c.chars().take(len - n).collect();
            next.push(format!("{keep}{commit}")); // applied
            next.push(format!("{c}{commit}")); // delete dropped
        }
        self.store(next);
    }
}

fn is_modifier_key(keyval: u32) -> bool {
    // Shift_L..Hyper_R, ISO_Level3_Shift and friends, Mode_switch.
    (0xffe1..=0xffee).contains(&keyval) || (0xfe01..=0xfe0f).contains(&keyval) || keyval == 0xff7e
}

impl Ime {
    pub fn new(cfg: &Config) -> Self {
        Ime {
            telex: Telex::new(cfg.telex.clone()),
            chord: Chord::default(),
            toggle_enabled: cfg.toggle_shortcut == ToggleShortcut::CtrlShift,
            mode: cfg.mode,
            caps: 0,
            purpose: 0,
            show: Show::Preedit,
            preedit_visible: false,
            app_id: None,
            has_selection: false,
            enabled: true,
            pending_cfg: None,
            surrounding_requested: false,
            model: FieldModel::default(),
        }
    }

    /// The app focused the field again: Surrounding Text is requested again.
    pub(crate) fn focus_in(&mut self) {
        self.surrounding_requested = false;
        self.model.clear();
        self.chord = Chord::default();
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool, out: &mut Vec<Out>) {
        if self.enabled && !enabled {
            self.flush_preedit(out);
            self.telex.reset();
        }
        self.enabled = enabled;
    }

    pub(crate) fn set_app_id(&mut self, app_id: Option<String>, cfg: &Config) {
        self.app_id = app_id;
        self.apply_config(cfg);
    }

    pub(crate) fn apply_config(&mut self, cfg: &Config) {
        self.toggle_enabled = cfg.toggle_shortcut == ToggleShortcut::CtrlShift;
        if self.telex.is_empty() && !self.preedit_visible {
            self.pending_cfg = None;
            self.telex.cfg = cfg.telex.clone();
            self.mode = cfg.mode_for_app(self.app_id.as_deref());
        } else {
            // Mid-word: applied by pick_show once the word has ended.
            self.pending_cfg = Some(cfg.clone());
        }
    }

    /// Fresh state after an internal error, keeping what the app told us.
    pub(crate) fn recover(&mut self, cfg: &Config) {
        let (caps, purpose, app_id, enabled) =
            (self.caps, self.purpose, self.app_id.clone(), self.enabled);
        *self = Ime::new(cfg);
        self.caps = caps;
        self.purpose = purpose;
        self.app_id = app_id;
        self.enabled = enabled;
        self.mode = cfg.mode_for_app(self.app_id.as_deref());
    }

    pub fn set_capabilities(&mut self, caps: u32) {
        self.caps = caps;
    }

    pub fn set_content_type(&mut self, purpose: u32, _hints: u32) {
        if self.purpose != purpose {
            self.model.clear();
        }
        self.purpose = purpose;
    }

    pub fn set_surrounding_text(
        &mut self,
        text: &str,
        cursor_pos: u32,
        anchor_pos: u32,
        out: &mut Vec<Out>,
    ) {
        // Only URL fields use Surrounding Text: the other fields never asked for it.
        if self.purpose != PURPOSE_URL {
            return;
        }
        let has_sel = cursor_pos != anchor_pos;
        self.has_selection = has_sel;
        if self.model.is_empty() {
            return;
        }
        // The report is the text up to the selection start: take the longest tail it ends with.
        let start = cursor_pos.min(anchor_pos) as usize;
        let before: String = text.chars().take(start).collect();
        let matched = self
            .model
            .candidates
            .iter()
            .filter(|c| !c.is_empty() && before.ends_with(c.as_str()))
            .max_by_key(|c| c.chars().count())
            .cloned();
        let Some(best) = matched else {
            // An older report than the keys already processed: nothing to check.
            return;
        };
        if best == self.model.want {
            // The word as typed is in the field, but an older report can match it too: keep the
            // other variants until a later report or a word boundary settles them.
            return;
        }
        if has_sel {
            // Chrome still shows a suggestion: wait for a report without it.
            return;
        }
        // A dropped delete left the stale tail: replace the whole tail with the word as typed.
        // The repair can be dropped too, so it is recorded as an edit of the stale tail.
        let len = best.chars().count();
        let want = self.model.want.clone();
        out.push(Out::DeleteSurrounding {
            offset: -(len as i32),
            nchars: len as u32,
        });
        out.push(Out::Commit(want.clone()));
        let mut stale = FieldModel::default();
        stale.store(vec![best]);
        stale.edit(len, &want);
        self.model = stale;
    }

    /// The display method is chosen when a word starts and kept until it ends. URL fields
    /// ask once per focus for Surrounding Text, which the repair in set_surrounding_text needs.
    fn pick_show(&mut self, out: &mut Vec<Out>) -> Show {
        if self.telex.is_empty() && !self.preedit_visible {
            if let Some(cfg) = self.pending_cfg.take() {
                self.telex.cfg = cfg.telex.clone();
                self.mode = cfg.mode_for_app(self.app_id.as_deref());
            }
            if self.purpose == PURPOSE_URL && !self.surrounding_requested {
                self.surrounding_requested = true;
                out.push(Out::RequireSurrounding);
            }
            let surrounding = self.caps & CAP_SURROUNDING_TEXT != 0;
            self.show = match self.mode {
                Mode::Preedit => Show::Preedit,
                Mode::Auto | Mode::Direct if surrounding => Show::Surrounding,
                Mode::Auto | Mode::Direct => Show::Preedit,
            };
        }
        self.show
    }

    fn replace(&mut self, backspaces: usize, text: String, out: &mut Vec<Out>) {
        if backspaces > 0 {
            out.push(Out::DeleteSurrounding {
                offset: -(backspaces as i32),
                nchars: backspaces as u32,
            });
        }
        if !text.is_empty() {
            out.push(Out::Commit(text.clone()));
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
    pub(crate) fn reset(&mut self, out: &mut Vec<Out>) {
        self.chord = Chord::default();
        self.hide_preedit(out);
        self.telex.reset();
        self.has_selection = false;
        self.model.clear();
    }

    pub fn process_key(
        &mut self,
        keyval: u32,
        _keycode: u32,
        state: u32,
        out: &mut Vec<Out>,
    ) -> bool {
        // The chord is tracked even while Vietnamese is off, so the toggle can turn it back on.
        if self.chord.observe(keyval, state) && self.toggle_enabled {
            out.push(Out::ToggleVietnamese);
        }
        if !self.enabled || state & RELEASE_MASK != 0 || is_modifier_key(keyval) {
            return false;
        }
        let shortcut = CONTROL_MASK | MOD1_MASK | MOD4_MASK | SUPER_MASK | HYPER_MASK | META_MASK;
        if state & shortcut != 0 || self.purpose == PURPOSE_PASSWORD || self.purpose == PURPOSE_PIN
        {
            self.flush_preedit(out);
            self.telex.reset();
            self.model.clear();
            return false;
        }

        let show = self.pick_show(out);
        if keyval == KEY_BACKSPACE {
            return self.backspace(show, out);
        }
        if (0x20..=0x7e).contains(&keyval) {
            let ch = char::from_u32(keyval).unwrap();
            return match show {
                Show::Preedit => self.printable_preedit(ch, out),
                _ => {
                    let url_tail = self.purpose == PURPOSE_URL && show == Show::Surrounding;
                    let r = self.telex.key(ch);
                    if !r.handled {
                        if url_tail {
                            self.model.native(ch);
                            self.settle_url_model(ch);
                        }
                        return false;
                    }
                    // An auto-restore commit can end in the boundary char too.
                    let boundary = r.commit.chars().last().filter(|b| !b.is_alphabetic());
                    if url_tail {
                        self.model.edit(r.backspaces, &r.commit);
                    }
                    self.replace(r.backspaces, r.commit, out);
                    if let (true, Some(b)) = (url_tail, boundary) {
                        self.settle_url_model(b);
                    }
                    true
                }
            };
        }
        // Enter, Tab, Escape, arrows, F-keys, keypad...: end the word and let the app have the key.
        self.flush_preedit(out);
        self.telex.reset();
        self.model.clear();
        false
    }

    fn backspace(&mut self, show: Show, out: &mut Vec<Out>) -> bool {
        // Backspace shrinks the tail back to an earlier state, so a late report of that state
        // would look like a dropped delete and a repair would erase good text. Stop tracking.
        self.model.clear();
        if self.purpose == PURPOSE_URL && self.has_selection {
            // Omnibox: native Backspace removes the selected suggestion; the typed text stays.
            self.has_selection = false;
            return false;
        }
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

    /// Settles the tracked tail at a word boundary, once the telex word is empty.
    fn settle_url_model(&mut self, boundary: char) {
        if self.telex.is_empty() {
            self.model.settle(boundary);
        }
    }

    fn printable_preedit(&mut self, ch: char, out: &mut Vec<Out>) -> bool {
        let before = self.telex.displayed();
        let r = self.telex.key(ch);
        // A VNI digit that was consumed as a mark keeps the word open.
        if ch.is_alphabetic() || (r.handled && !self.telex.is_empty()) {
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
            let kv = if ch == '\u{8}' {
                KEY_BACKSPACE
            } else if ch == '\n' {
                KEY_RETURN
            } else {
                ch as u32
            };
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
        assert!(!out
            .iter()
            .any(|o| matches!(o, Out::DeleteSurrounding { .. })));
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

    /// A URL field (Omnibox): Surrounding Text is reported, but purpose is URL.
    fn url_field() -> Ime {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9 | CAP_SURROUNDING_TEXT);
        ime.set_content_type(PURPOSE_URL, 0);
        ime
    }

    /// A URL field with no Surrounding Text: composes in preedit.
    fn preedit_field() -> Ime {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9);
        ime.set_content_type(PURPOSE_URL, 0);
        ime
    }

    /// Omnibox as far as it matters: the text before the caret and an inline suggestion selected
    /// after it. Chrome drops a delete while a selection exists, a commit replaces the selection,
    /// and a report gives cursor = caret, anchor = end of the suggestion.
    struct Omnibox {
        text: String,
        sugg: String,
        requested: bool,
        table: &'static [(&'static str, &'static str)],
        /// Report after every key (true) or only when the test asks for it.
        report_each_key: bool,
        /// The next delete is dropped, as if a selection had reappeared.
        drop_next_delete: bool,
    }

    impl Omnibox {
        fn new(table: &'static [(&'static str, &'static str)]) -> Self {
            Omnibox {
                text: String::new(),
                sugg: String::new(),
                requested: false,
                table,
                report_each_key: true,
                drop_next_delete: false,
            }
        }

        fn suggest(&mut self) {
            self.sugg = self
                .table
                .iter()
                .find(|(k, _)| Some(*k) == self.text.rsplit(' ').next())
                .map(|(_, v)| v.to_string())
                .unwrap_or_default();
        }

        fn apply(&mut self, outs: &[Out]) {
            for o in outs {
                match o {
                    Out::Commit(t) => {
                        self.sugg.clear();
                        self.text.push_str(t);
                    }
                    Out::DeleteSurrounding { nchars, .. } => {
                        if self.drop_next_delete {
                            self.drop_next_delete = false;
                        } else if self.sugg.is_empty() {
                            // With a selection Chrome drops the delete.
                            for _ in 0..*nchars {
                                self.text.pop();
                            }
                        }
                    }
                    Out::RequireSurrounding => self.requested = true,
                    other => panic!("unexpected {other:?}"),
                }
            }
        }

        fn key(&mut self, ime: &mut Ime, ch: char) {
            let mut out = Vec::new();
            if !ime.process_key(ch as u32, 0, 0, &mut out) {
                self.sugg.clear(); // typed text replaces the selection
                self.text.push(ch);
            }
            self.apply(&out);
            self.suggest();
            if self.report_each_key {
                self.report(ime);
            }
        }

        /// One Surrounding Text report of the current state. Returns what the engine did with it.
        fn report(&mut self, ime: &mut Ime) -> Vec<Out> {
            if !self.requested {
                return Vec::new();
            }
            let caret = self.text.chars().count() as u32;
            let anchor = caret + self.sugg.chars().count() as u32;
            let full = format!("{}{}", self.text, self.sugg);
            let mut outs = Vec::new();
            ime.set_surrounding_text(&full, caret, anchor, &mut outs);
            self.apply(&outs);
            self.suggest();
            outs
        }

        fn type_text(&mut self, ime: &mut Ime, s: &str) {
            for ch in s.chars() {
                self.key(ime, ch);
            }
        }

        /// All keys first, then one late report, then one more. Returns what the first did.
        fn type_delayed(&mut self, ime: &mut Ime, s: &str) -> Vec<Out> {
            self.report_each_key = false;
            for ch in s.chars() {
                self.key(ime, ch);
            }
            let first = self.report(ime);
            self.report(ime);
            first
        }
    }

    const OMNI: &[(&str, &str)] = &[
        ("Ru", "st-lang.org"),
        ("Tra", "nslate.google.com"),
        ("cu", "ong.com"),
        ("ngu", "oi.vn"),
    ];

    #[test]
    fn url_field_rus_with_late_report_becomes_ru() {
        let mut ime = url_field();
        let mut f = Omnibox::new(OMNI);
        f.type_delayed(&mut ime, "Rus");
        assert_eq!(f.text, "Rú");
    }

    #[test]
    fn url_field_traanf_with_late_report_becomes_tran() {
        let mut ime = url_field();
        let mut f = Omnibox::new(OMNI);
        f.type_delayed(&mut ime, "Traanf");
        assert_eq!(f.text, "Trần");
    }

    #[test]
    fn url_field_trailing_space_with_late_report_keeps_space() {
        let mut ime = url_field();
        let mut f = Omnibox::new(OMNI);
        f.type_delayed(&mut ime, "Traanf ");
        assert_eq!(f.text, "Trần ");
    }

    #[test]
    fn url_field_cuwar_with_late_report_becomes_cua() {
        let mut ime = url_field();
        let mut f = Omnibox::new(OMNI);
        f.type_delayed(&mut ime, "cuwar");
        assert_eq!(f.text, "cửa");
    }

    #[test]
    fn url_field_nguwowif_with_late_report_becomes_nguoi() {
        let mut ime = url_field();
        let mut f = Omnibox::new(OMNI);
        f.type_delayed(&mut ime, "nguwowif");
        assert_eq!(f.text, "người");
    }

    #[test]
    fn url_field_report_after_every_key_converges() {
        for (input, want) in [
            ("Rus", "Rú"),
            ("Traanf", "Trần"),
            ("cuwar", "cửa"),
            ("nguwowif", "người"),
        ] {
            let mut ime = url_field();
            let mut f = Omnibox::new(OMNI);
            f.type_text(&mut ime, input);
            assert_eq!(f.text, want, "typing {input}");
        }
    }

    #[test]
    fn url_field_no_suggestions_is_not_repaired() {
        let mut ime = url_field();
        let mut f = Omnibox::new(&[]);
        let first = f.type_delayed(&mut ime, "Traanf");
        assert_eq!(f.text, "Trần");
        assert!(first.is_empty());
        let mut ime = url_field();
        let mut f = Omnibox::new(&[]);
        let first = f.type_delayed(&mut ime, "xus");
        assert_eq!(f.text, "xú");
        assert!(first.is_empty());
    }

    #[test]
    fn url_field_old_report_matches_nothing() {
        let mut ime = url_field();
        keys(&mut ime, "Rus");
        let before = ime.model.candidates.clone();
        let mut out = Vec::new();
        ime.set_surrounding_text("Zzz", 3, 3, &mut out);
        assert!(out.is_empty());
        assert_eq!(ime.model.candidates, before);
    }

    #[test]
    fn url_field_dropped_repair_is_repaired_again() {
        let mut ime = url_field();
        let mut f = Omnibox::new(OMNI);
        f.report_each_key = false;
        for ch in "Rus".chars() {
            f.key(&mut ime, ch);
        }
        assert_eq!(f.text, "Ruú");
        f.drop_next_delete = true;
        f.report(&mut ime); // the repair is dropped
        f.report(&mut ime); // and repeated
        assert_eq!(f.text, "Rú");
    }

    #[test]
    fn url_field_model_stays_bounded() {
        let mut ime = url_field();
        let mut out = Vec::new();
        for ch in "nguwowif".repeat(8).chars() {
            ime.process_key(ch as u32, 0, 0, &mut out);
            assert!(ime.model.candidates.len() <= MODEL_MAX_CANDIDATES);
            assert!(ime
                .model
                .candidates
                .iter()
                .all(|c| c.chars().count() <= MODEL_MAX_CHARS));
        }
    }

    #[test]
    fn url_field_keeps_only_two_words_tracked() {
        // No reports at all: the variants can only grow with the edits, so the boundaries must
        // bound them. Twelve words with marks, and the model stays small after each one.
        let mut ime = url_field();
        let mut out = Vec::new();
        for _ in 0..12 {
            for ch in "nguwowif ".chars() {
                ime.process_key(ch as u32, 0, 0, &mut out);
            }
            assert!(
                ime.model.candidates.len() <= 64,
                "{}",
                ime.model.candidates.len()
            );
        }
    }

    #[test]
    fn url_field_two_word_burst_with_one_late_report_repairs_both() {
        let mut ime = url_field();
        let mut f = Omnibox::new(OMNI);
        f.type_delayed(&mut ime, "Rus Traanf");
        assert_eq!(f.text, "Rú Trần");
    }

    #[test]
    fn url_field_never_shows_preedit() {
        // Omnibox::apply panics on any Preedit.
        let mut ime = url_field();
        let mut f = Omnibox::new(OMNI);
        f.type_text(&mut ime, "Traanf Rus ");
        assert_eq!(f.text, "Trần Rú ");
    }

    #[test]
    fn url_field_enter_is_native() {
        let mut ime = url_field();
        keys(&mut ime, "Rus");
        let mut out = Vec::new();
        assert!(!ime.process_key(KEY_RETURN, 0, 0, &mut out));
        assert!(out.is_empty());
    }

    #[test]
    fn url_field_backspace_on_selection_is_native() {
        let mut ime = url_field();
        keys(&mut ime, "Ru");
        let mut out = Vec::new();
        ime.set_surrounding_text("Rust-lang.org", 2, 13, &mut out);
        assert!(out.is_empty());
        let mut out = Vec::new();
        assert!(!ime.process_key(KEY_BACKSPACE, 0, 0, &mut out));
        assert!(out.is_empty());
        assert_eq!(ime.telex.displayed(), "Ru");
        assert!(!ime.has_selection);
    }

    #[test]
    fn url_field_requires_surrounding_once_per_focus() {
        let count = |out: &[Out]| {
            out.iter()
                .filter(|o| **o == Out::RequireSurrounding)
                .count()
        };
        let mut ime = url_field();
        let mut out = Vec::new();
        for ch in "Rus".chars() {
            ime.process_key(ch as u32, 0, 0, &mut out);
        }
        assert_eq!(count(&out), 1);
        keys(&mut ime, "\n"); // word ends
        ime.focus_in();
        let mut out = Vec::new();
        ime.process_key('a' as u32, 0, 0, &mut out);
        assert_eq!(count(&out), 1);
        let mut out = Vec::new();
        ime.process_key('b' as u32, 0, 0, &mut out);
        assert_eq!(count(&out), 0);
    }

    #[test]
    fn non_url_fields_ignore_surrounding_text() {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9 | CAP_SURROUNDING_TEXT);
        let mut out = Vec::new();
        ime.set_surrounding_text("Rú", 2, 0, &mut out);
        assert!(out.is_empty());
        assert!(!ime.has_selection);
    }

    #[test]
    fn non_url_boundary_hides_before_commit() {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9);
        keys(&mut ime, "Rus");
        let out = keys(&mut ime, " ");
        assert_eq!(out, vec![Out::HidePreedit, Out::Commit("Rú ".into())]);
    }

    #[test]
    fn selection_autocomplete_keeps_composition() {
        let mut ime = preedit_field();
        let mut out = Vec::new();
        // Caret at 1, then type 'g'.
        ime.set_surrounding_text("g", 1, 1, &mut out);
        ime.process_key('g' as u32, 0, 0, &mut out);
        // Omnibox selects the suggestion "oogle.com" that starts at the caret (cursor 10, anchor 1).
        ime.set_surrounding_text("google.com", 10, 1, &mut out);
        out.clear();
        // The selection start is still the word's end, so 'o' extends "g" instead of resetting.
        ime.process_key('o' as u32, 0, 0, &mut out);
        assert_eq!(out, vec![Out::Preedit("go".into())]);
    }

    #[test]
    fn autocomplete_selection_at_caret_keeps_word() {
        for (cursor, anchor) in [(3, 9), (9, 3)] {
            let mut ime = preedit_field();
            let mut out = Vec::new();
            ime.set_surrounding_text("Tra", 3, 3, &mut out);
            keys(&mut ime, "Tra");
            out.clear();
            ime.set_surrounding_text("Translate", cursor, anchor, &mut out);
            assert!(out.is_empty(), "selection reset the word: {out:?}");
            let out = keys(&mut ime, "anf");
            assert_eq!(out.last(), Some(&Out::Preedit("Trần".into())));
        }
    }

    #[test]
    fn vni_digits_in_preedit_keep_word() {
        let mut cfg = Config::default();
        cfg.telex.input_method = rustype_core::telex::InputMethod::Vni;
        let mut ime = Ime::new(&cfg);
        ime.set_capabilities(0x9);
        let out = keys(&mut ime, "ba1n ");
        assert_eq!(out.last(), Some(&Out::Commit("bán ".into())));
    }

    #[test]
    fn config_change_mid_word_applies_after_word() {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9);
        let out = keys(&mut ime, "ba");
        assert_eq!(out.last(), Some(&Out::Preedit("ba".into())));
        let mut vni = Config::default();
        vni.telex.input_method = rustype_core::telex::InputMethod::Vni;
        ime.apply_config(&vni);
        let out = keys(&mut ime, " ");
        assert_eq!(out, vec![Out::HidePreedit, Out::Commit("ba ".into())]);
        let out = keys(&mut ime, "ba1n ");
        assert_eq!(out.last(), Some(&Out::Commit("bán ".into())));
    }

    #[test]
    fn recover_keeps_enabled_and_app_mode() {
        let mut cfg = Config::default();
        cfg.app_modes.insert("kitty".into(), Mode::Preedit);
        let mut ime = Ime::new(&cfg);
        ime.set_app_id(Some("kitty".into()), &cfg);
        let mut out = Vec::new();
        ime.set_enabled(false, &mut out);
        ime.recover(&cfg);
        assert!(!ime.is_enabled());
        assert_eq!(ime.mode, Mode::Preedit);
    }

    #[test]
    fn disabled_state_bypasses() {
        let mut ime = Ime::new(&Config::default());
        let mut out = Vec::new();
        ime.set_enabled(false, &mut out);
        assert!(!ime.is_enabled());
        let handled = ime.process_key('w' as u32, 0, 0, &mut out);
        assert!(!handled);
        assert!(out.is_empty());
    }

    const KEY_CTRL_L: u32 = 0xffe3;
    const KEY_CTRL_R: u32 = 0xffe4;
    const KEY_SHIFT_L: u32 = 0xffe1;
    const KEY_SHIFT_R: u32 = 0xffe2;
    const KEY_ALT_L: u32 = 0xffe9;
    const KEY_LEFT: u32 = 0xff51;

    /// One key event, press or release, with the modifier bits in its state.
    fn ev(ime: &mut Ime, kv: u32, up: bool, mods: u32) -> Vec<Out> {
        let mut out = Vec::new();
        let state = mods | if up { RELEASE_MASK } else { 0 };
        ime.process_key(kv, 0, state, &mut out);
        out
    }

    fn press(ime: &mut Ime, kv: u32) -> Vec<Out> {
        ev(ime, kv, false, 0)
    }

    fn release(ime: &mut Ime, kv: u32) -> Vec<Out> {
        ev(ime, kv, true, 0)
    }

    fn toggles(outs: &[Out]) -> usize {
        outs.iter().filter(|o| **o == Out::ToggleVietnamese).count()
    }

    fn engine() -> Ime {
        Ime::new(&Config::default())
    }

    #[test]
    fn ctrl_shift_toggles_in_either_order_and_side() {
        for (a, b) in [
            (KEY_CTRL_L, KEY_SHIFT_L),
            (KEY_SHIFT_L, KEY_CTRL_L),
            (KEY_CTRL_R, KEY_SHIFT_R),
            (KEY_SHIFT_R, KEY_CTRL_R),
            (KEY_CTRL_L, KEY_SHIFT_R),
            (KEY_SHIFT_R, KEY_CTRL_L),
        ] {
            let mut ime = engine();
            let mut n = toggles(&press(&mut ime, a));
            n += toggles(&press(&mut ime, b));
            n += toggles(&release(&mut ime, a));
            n += toggles(&release(&mut ime, b));
            assert_eq!(n, 1, "chord {a:#x} then {b:#x}");
        }
        // Releasing the second key first fires too.
        let mut ime = engine();
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        assert_eq!(toggles(&release(&mut ime, KEY_SHIFT_L)), 1);
    }

    #[test]
    fn modifier_events_pass_through_to_the_app() {
        let mut ime = engine();
        let mut handled = false;
        for (kv, up) in [
            (KEY_CTRL_L, false),
            (KEY_SHIFT_L, false),
            (KEY_SHIFT_L, true),
            (KEY_CTRL_L, true),
        ] {
            let mut out = Vec::new();
            handled |= ime.process_key(kv, 0, if up { RELEASE_MASK } else { 0 }, &mut out);
        }
        assert!(!handled);
    }

    #[test]
    fn other_key_cancels_the_chord() {
        // Ctrl+Shift+Left and Ctrl+Shift+T are not the toggle.
        let mut ime = engine();
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        let mut n = toggles(&press(&mut ime, KEY_LEFT));
        n += toggles(&release(&mut ime, KEY_LEFT));
        n += toggles(&release(&mut ime, KEY_SHIFT_L));
        n += toggles(&release(&mut ime, KEY_CTRL_L));
        assert_eq!(n, 0);

        let mut ime = engine();
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        let mut n = toggles(&press(&mut ime, 0x74)); // 't'
        n += toggles(&release(&mut ime, 0x74));
        n += toggles(&release(&mut ime, KEY_CTRL_L));
        n += toggles(&release(&mut ime, KEY_SHIFT_L));
        assert_eq!(n, 0);
    }

    #[test]
    fn alt_or_super_cancels_the_chord_until_all_released() {
        let mut ime = engine();
        press(&mut ime, KEY_ALT_L);
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        let mut n = toggles(&release(&mut ime, KEY_SHIFT_L));
        n += toggles(&release(&mut ime, KEY_CTRL_L));
        assert_eq!(n, 0, "Alt held before the chord");

        // Alt is in the state of the Ctrl press, though its own press was never seen.
        let mut ime = engine();
        ev(&mut ime, KEY_CTRL_L, false, MOD1_MASK);
        press(&mut ime, KEY_SHIFT_L);
        assert_eq!(toggles(&release(&mut ime, KEY_SHIFT_L)), 0);

        // Super in the state of the release.
        let mut ime = engine();
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        assert_eq!(toggles(&ev(&mut ime, KEY_CTRL_L, true, MOD4_MASK)), 0);
    }

    #[test]
    fn key_between_the_two_modifiers_cancels() {
        let mut ime = engine();
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, 0x61); // 'a'
        release(&mut ime, 0x61);
        press(&mut ime, KEY_SHIFT_L);
        assert_eq!(toggles(&release(&mut ime, KEY_SHIFT_L)), 0);
        assert_eq!(toggles(&release(&mut ime, KEY_CTRL_L)), 0);
    }

    #[test]
    fn toggle_fires_while_vietnamese_is_off() {
        let mut ime = engine();
        ime.set_enabled(false, &mut Vec::new());
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        assert_eq!(toggles(&release(&mut ime, KEY_SHIFT_L)), 1);
        // And typing is not converted while disabled.
        let mut out = Vec::new();
        assert!(!ime.process_key(0x61, 0, 0, &mut out));
    }

    #[test]
    fn off_config_never_toggles() {
        let cfg = Config {
            toggle_shortcut: crate::config::ToggleShortcut::Off,
            ..Config::default()
        };
        let mut ime = Ime::new(&cfg);
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        assert_eq!(toggles(&release(&mut ime, KEY_SHIFT_L)), 0);
    }

    #[test]
    fn focus_and_reset_clear_the_chord() {
        let mut ime = engine();
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        ime.focus_in();
        assert_eq!(toggles(&release(&mut ime, KEY_SHIFT_L)), 0);
        release(&mut ime, KEY_CTRL_L);

        let mut ime = engine();
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        ime.reset(&mut Vec::new());
        assert_eq!(toggles(&release(&mut ime, KEY_CTRL_L)), 0);

        // After the clear, a new chord still works.
        press(&mut ime, KEY_CTRL_L);
        press(&mut ime, KEY_SHIFT_L);
        assert_eq!(toggles(&release(&mut ime, KEY_SHIFT_L)), 1);
    }

    #[test]
    fn key_repeat_fires_once() {
        let mut ime = engine();
        let mut n = toggles(&press(&mut ime, KEY_CTRL_L));
        n += toggles(&press(&mut ime, KEY_SHIFT_L));
        n += toggles(&press(&mut ime, KEY_SHIFT_L)); // auto-repeat
        n += toggles(&press(&mut ime, KEY_CTRL_L)); // auto-repeat
        n += toggles(&release(&mut ime, KEY_SHIFT_L));
        n += toggles(&press(&mut ime, KEY_SHIFT_L)); // pressed again while Ctrl is still down
        n += toggles(&release(&mut ime, KEY_SHIFT_L));
        n += toggles(&release(&mut ime, KEY_CTRL_L));
        assert_eq!(n, 1);
    }
}

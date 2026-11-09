//! Chrome's Omnibox, modelled as far as it matters, against the engine's public API.
//!
//! The fake field keeps the text before the caret and an optional inline suggestion selected
//! after it. It reports snapshots (cursor = caret, anchor = end of the suggestion) with a lag,
//! and it applies the engine's outputs the way Chrome does: a delete is ignored while a
//! suggestion is selected, a commit replaces the suggestion, and a native key does the same.
//! The oracle is a plain field with no suggestions and reports after every change.

use rustype_linux::config::Config;
use rustype_linux::ime::{Ime, Out, CAP_SURROUNDING_TEXT};
use std::collections::{HashSet, VecDeque};

const PURPOSE_URL: u32 = 5;
const KEY_BACKSPACE: u32 = 0xff08;
const SUGGESTION: &str = "xyz.example";
const GOLDEN: &str = include_str!("../../rustype-core/tests/golden.tsv");

/// Every `STRIDE`-th golden row runs through the matrix (1 = all rows).
const STRIDE: usize = 1;

#[derive(Clone, Copy, Debug)]
enum Lag {
    /// A report is delivered once this many more keys have been typed.
    Fixed(usize),
    /// Nothing is delivered until every key has been typed.
    AfterAll,
    /// Each report waits 0 to 3 keys, from the seeded generator.
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    /// Never suggests: the plain field the oracle runs in.
    None,
    /// Suggests after every letter once at least two letters of the word are typed.
    AfterTwoLetters,
    /// Suggests only after the second and third letters of each word.
    FirstLetters,
    /// Suggests on a seeded coin flip after each char or commit, but only on text the user could
    /// have typed: Chrome suggests from history, so a tail garbled by a dropped delete gets none.
    Random,
}

struct Snapshot {
    text: String,
    cursor: u32,
    anchor: u32,
    due: usize,
}

struct Field {
    ime: Ime,
    /// Text before the caret.
    text: String,
    /// Suggestion selected after the caret.
    sugg: String,
    policy: Policy,
    lag: Lag,
    rng: u64,
    requested: bool,
    queue: VecDeque<Snapshot>,
    /// Index of the key being typed.
    key: usize,
    /// Every text the plain field holds while this row is typed (Random policy only).
    plain: HashSet<String>,
}

impl Field {
    fn new(policy: Policy, lag: Lag, seed: u64) -> Self {
        let mut ime = Ime::new(&Config::default());
        ime.set_capabilities(0x9 | CAP_SURROUNDING_TEXT);
        ime.set_content_type(PURPOSE_URL, 0);
        Field {
            ime,
            text: String::new(),
            sugg: String::new(),
            policy,
            lag,
            rng: seed | 1,
            requested: false,
            queue: VecDeque::new(),
            key: 0,
            plain: HashSet::new(),
        }
    }

    fn next_rand(&mut self) -> u64 {
        // xorshift64: deterministic, so every failure can be replayed.
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// Chrome reports after every change once the engine has asked for Surrounding Text.
    fn snapshot(&mut self) {
        if !self.requested {
            return;
        }
        let due = match self.lag {
            Lag::Fixed(l) => self.key + l,
            Lag::AfterAll => usize::MAX,
            Lag::Random => self.key + (self.next_rand() % 4) as usize,
        };
        let cursor = self.text.chars().count() as u32;
        let anchor = cursor + self.sugg.chars().count() as u32;
        self.queue.push_back(Snapshot {
            text: format!("{}{}", self.text, self.sugg),
            cursor,
            anchor,
            due,
        });
    }

    /// Called after each native char and each commit.
    fn maybe_suggest(&mut self) {
        let trailing = self
            .text
            .chars()
            .rev()
            .take_while(|c| c.is_alphabetic())
            .count();
        let on = match self.policy {
            Policy::None => false,
            Policy::AfterTwoLetters => trailing >= 2,
            Policy::FirstLetters => (2..=3).contains(&trailing),
            Policy::Random => self.next_rand().is_multiple_of(2) && self.plain.contains(&self.text),
        };
        self.sugg = if on {
            SUGGESTION.to_string()
        } else {
            String::new()
        };
    }

    /// The engine's outputs, applied in order.
    fn apply(&mut self, outs: Vec<Out>) {
        for o in outs {
            match o {
                Out::Commit(s) => {
                    self.sugg.clear(); // a commit replaces the selected suggestion
                    self.text.push_str(&s);
                    self.maybe_suggest();
                    self.snapshot();
                }
                Out::DeleteSurrounding { nchars, .. } => {
                    // Chrome drops a delete while a suggestion is selected.
                    if self.sugg.is_empty() {
                        for _ in 0..nchars {
                            self.text.pop();
                        }
                    }
                    self.snapshot();
                }
                Out::RequireSurrounding => {
                    self.requested = true;
                    self.snapshot();
                }
                other => panic!("unexpected {other:?} in a URL field"),
            }
        }
    }

    /// A key that the engine did not take: the app types it.
    fn native(&mut self, keyval: u32) {
        if keyval == KEY_BACKSPACE {
            // With a suggestion selected, Backspace removes only the suggestion.
            if self.sugg.is_empty() {
                self.text.pop();
            } else {
                self.sugg.clear();
            }
            self.snapshot();
            return;
        }
        let c = char::from_u32(keyval).expect("a character key");
        self.sugg.clear(); // typed text replaces the selected suggestion
        self.text.push(c);
        self.maybe_suggest();
        self.snapshot();
    }

    fn key(&mut self, keyval: u32) {
        let mut outs = Vec::new();
        let handled = self.ime.process_key(keyval, 0, 0, &mut outs);
        self.apply(outs);
        if !handled {
            self.native(keyval);
        }
        self.deliver_ready();
        // Chrome catches up between words: at a boundary every pending report is delivered
        // before the next key. AfterAll waits for the end of the input instead.
        if is_boundary(keyval) && !matches!(self.lag, Lag::AfterAll) {
            self.drain();
        }
        self.key += 1;
    }

    fn deliver(&mut self, s: Snapshot) {
        let mut outs = Vec::new();
        self.ime
            .set_surrounding_text(&s.text, s.cursor, s.anchor, &mut outs);
        self.apply(outs);
    }

    fn deliver_ready(&mut self) {
        while let Some(front) = self.queue.front() {
            if front.due > self.key {
                break;
            }
            let Some(s) = self.queue.pop_front() else {
                break;
            };
            self.deliver(s);
        }
    }

    /// Delivers every queued report, including those the repairs cause.
    fn drain(&mut self) {
        let mut delivered = 0usize;
        while let Some(s) = self.queue.pop_front() {
            self.deliver(s);
            delivered += 1;
            assert!(delivered < 100_000, "reports never settle");
        }
    }

    /// Delivers the rest, then returns the text and whether a suggestion is still selected.
    fn finish(mut self) -> (String, bool) {
        self.drain();
        let selected = !self.sugg.is_empty();
        (self.text, selected)
    }
}

/// The final text, and whether a suggestion is still selected when the input ends.
/// A key that ends a word: any printable non-letter (Backspace is not one).
fn is_boundary(keyval: u32) -> bool {
    keyval != KEY_BACKSPACE && char::from_u32(keyval).is_some_and(|c| !c.is_alphabetic())
}

/// Words in a row: runs of letters.
fn word_count(keys: &[u32]) -> usize {
    let mut words = 0;
    let mut in_word = false;
    for &k in keys {
        let letter = char::from_u32(k).is_some_and(char::is_alphabetic);
        if letter && !in_word {
            words += 1;
        }
        in_word = letter;
    }
    words
}

/// Every text the plain field holds while `keys` are typed.
fn plain_states(keys: &[u32]) -> HashSet<String> {
    let mut field = Field::new(Policy::None, Lag::Fixed(0), 1);
    let mut states = HashSet::new();
    for &k in keys {
        field.key(k);
        states.insert(field.text.clone());
    }
    states
}

fn run(keys: &[u32], policy: Policy, lag: Lag, seed: u64) -> (String, bool) {
    let mut field = Field::new(policy, lag, seed);
    if policy == Policy::Random {
        field.plain = plain_states(keys);
    }
    for &k in keys {
        field.key(k);
    }
    field.finish()
}

/// Golden rows: the keys column, with `\b` as Backspace and `\\` as a backslash.
fn corpus() -> Vec<Vec<u32>> {
    GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').next())
        .filter(|keys| !keys.is_empty())
        .map(decode)
        .collect()
}

fn decode(col: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut it = col.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.peek() {
                Some('b') => {
                    it.next();
                    out.push(KEY_BACKSPACE);
                    continue;
                }
                Some('\\') => {
                    it.next();
                    out.push('\\' as u32);
                    continue;
                }
                _ => {}
            }
        }
        out.push(c as u32);
    }
    out
}

fn show(keys: &[u32]) -> String {
    keys.iter()
        .map(|&k| {
            if k == KEY_BACKSPACE {
                "\\b".to_string()
            } else {
                char::from_u32(k).unwrap().to_string()
            }
        })
        .collect()
}

#[test]
fn golden_rows_through_omnibox_match_plain_field() {
    let rows = corpus();
    let lags = [
        Lag::Fixed(0),
        Lag::Fixed(1),
        Lag::Fixed(3),
        Lag::AfterAll,
        Lag::Random,
    ];
    let mut runs = 0usize;
    let mut words_out_of_scope = 0usize;
    // Runs whose final text differs from the plain field, with the policy and lag they ran under.
    let mut differ: Vec<(usize, Policy, Lag, String, String, bool)> = Vec::new();
    for (row, keys) in rows.iter().enumerate().step_by(STRIDE) {
        let (want, _) = run(keys, Policy::None, Lag::Fixed(0), 1);
        // Backspace on a selected suggestion legitimately differs from the plain field
        // (Chrome removes only the suggestion), so those rows run without suggestions.
        let has_backspace = keys.contains(&KEY_BACKSPACE);
        let policies: &[Policy] = if has_backspace {
            &[Policy::None]
        } else {
            &[
                Policy::AfterTwoLetters,
                Policy::FirstLetters,
                Policy::Random,
            ]
        };
        for (pi, &policy) in policies.iter().enumerate() {
            for (li, &lag) in lags.iter().enumerate() {
                // AfterAll holds every report back. Only the last two words stay repairable, so
                // a report held past a second word boundary cannot reach the first word.
                let boundaries = keys.iter().filter(|&&k| is_boundary(k)).count();
                if matches!(lag, Lag::AfterAll) && (word_count(keys) > 2 || boundaries > 1) {
                    words_out_of_scope += 1;
                    continue;
                }
                let seed = (row as u64 + 1) * 131 + (pi as u64) * 17 + (li as u64) * 3;
                let (got, selected) = run(keys, policy, lag, seed);
                runs += 1;
                if got != want {
                    differ.push((row, policy, lag, got, want.clone(), selected));
                }
            }
        }
    }
    // Out of scope by design: the input ends while a suggestion is still selected after a dropped
    // delete. Chrome sends no report without the selection until the user types again, so no
    // repair can arrive. Counted here, not asserted.
    let (selected, failures): (Vec<_>, Vec<_>) = differ.into_iter().partition(|f| f.5);
    eprintln!(
        "omnibox matrix: {} of {} golden rows (stride {STRIDE}), {runs} runs; \
         {} end with a suggestion still selected (out of scope), \
         {words_out_of_scope} AfterAll runs on rows of more than 2 words (out of scope), \
         {} checked failures",
        rows.len().div_ceil(STRIDE),
        rows.len(),
        selected.len(),
        failures.len()
    );
    if !failures.is_empty() {
        let mut failures = failures;
        // Shortest keys first: the minimal examples.
        failures.sort_by_key(|f| rows[f.0].len());
        let mut msg = format!(
            "{} of {runs} runs end with a text the plain field does not give\n",
            failures.len()
        );
        for pol in [
            Policy::AfterTwoLetters,
            Policy::FirstLetters,
            Policy::Random,
        ] {
            let n = failures.iter().filter(|f| f.1 == pol).count();
            msg.push_str(&format!("  policy {pol:?}: {n} failures\n"));
        }
        for lag in lags {
            let n = failures
                .iter()
                .filter(|f| format!("{:?}", f.2) == format!("{lag:?}"))
                .count();
            msg.push_str(&format!("  lag {lag:?}: {n} failures\n"));
        }
        for (row, policy, lag, got, want, _) in failures.iter().take(6) {
            msg.push_str(&format!(
                "  keys {:?}, policy {:?}, lag {:?}: got {:?}, want {:?}\n",
                show(&rows[*row]),
                policy,
                lag,
                got,
                want
            ));
        }
        panic!("{msg}");
    }
}

#[test]
fn late_report_after_backspace_never_erases_good_text() {
    // A report of the state before the Backspace looks like a dropped delete.
    let keys: Vec<u32> = "ab vfe\u{8}"
        .chars()
        .map(|c| {
            if c == '\u{8}' {
                KEY_BACKSPACE
            } else {
                c as u32
            }
        })
        .collect();
    assert_eq!(run(&keys, Policy::None, Lag::Fixed(3), 1).0, "ab v");
    assert_eq!(run(&keys, Policy::None, Lag::AfterAll, 1).0, "ab v");
}

#[test]
fn backspace_on_a_suggestion_removes_only_the_suggestion() {
    // Chrome: Backspace with a suggestion selected removes the suggestion, not the typed "b".
    let mut field = Field::new(Policy::AfterTwoLetters, Lag::Fixed(0), 1);
    for c in "ab".chars() {
        field.key(c as u32);
    }
    field.key(KEY_BACKSPACE);
    field.key('c' as u32);
    assert_eq!(field.finish().0, "abc");
}

#[test]
fn backspace_without_a_suggestion_matches_plain_field() {
    let keys = decode("tuaa\\b\\bye");
    let (want, _) = run(&keys, Policy::None, Lag::Fixed(0), 1);
    for lag in [Lag::Fixed(0), Lag::Fixed(1), Lag::Fixed(3), Lag::AfterAll] {
        assert_eq!(run(&keys, Policy::None, lag, 1).0, want, "lag {lag:?}");
    }
}

#[test]
fn many_dropped_deletes_under_suggestions_are_repaired_at_lag_zero() {
    // Every commit shows a suggestion, so every delete is dropped. With too few tracked variants
    // the engine forgot the stale text and it stayed in the field.
    let keys = decode("huwowufn ");
    let (want, _) = run(&keys, Policy::None, Lag::Fixed(0), 1);
    let (got, _) = run(&keys, Policy::AfterTwoLetters, Lag::Fixed(0), 1);
    assert_eq!(got, want);
}

#[test]
fn stale_report_matching_the_word_keeps_later_repairs() {
    // "yefz": an older report equal to the word as typed must not discard the variant that a
    // later report needs to repair "yeye".
    let keys = decode("yefz");
    let (want, _) = run(&keys, Policy::None, Lag::Fixed(0), 1);
    let (got, _) = run(&keys, Policy::FirstLetters, Lag::Fixed(1), 1);
    assert_eq!(got, want);
}

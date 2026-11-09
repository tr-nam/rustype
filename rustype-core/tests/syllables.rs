//! Syllable-level spelling check through the public API only.
//!
//! Every syllable in a curated Vietnamese inventory (initial + rime + tone) is typed in
//! Telex and VNI in several key orders, and must come out with the expected spelling in
//! both orthographies. The inventory and tone rules are written here, not taken from the
//! engine tables, so the test can catch table mistakes. Deterministic and fast.

use rustype_core::telex::{Config, InputMethod, Telex};

const INITIALS: &[&str] = &[
    "", "b", "c", "ch", "d", "đ", "g", "gh", "gi", "h", "k", "kh", "l", "m", "n", "ng", "ngh",
    "nh", "p", "ph", "qu", "r", "s", "t", "th", "tr", "v", "x",
];

const NUCLEI: &[&str] = &[
    "a", "ă", "â", "e", "ê", "i", "o", "ô", "ơ", "u", "ư", "y", "ai", "ao", "au", "ay", "âu", "ây",
    "eo", "êu", "ia", "iê", "iêu", "iu", "oa", "oe", "oai", "oao", "oay", "oeo", "oi", "ôi", "ơi",
    "ua", "ui", "uy", "uya", "uyu", "uyê", "uê", "uôi", "uơ", "uây", "uô", "ưa", "ưi", "ưu", "ươ",
    "ươi", "ươu", "yê", "yêu", "ya", "oă", "uă", "uâ",
];

/// Nuclei that never stand open (no final consonant).
const NEEDS_FINAL: &[&str] = &["ă", "â", "iê", "oă", "uă", "uâ", "uô", "uyê", "yê", "ươ"];

/// Real closed rimes, nucleus -> final consonants.
fn closed_ok(nuc: &str, fin: &str) -> bool {
    let set: &[&str] = match nuc {
        "a" => &["c", "ch", "m", "n", "ng", "nh", "p", "t"],
        "ă" | "â" | "o" | "ô" | "u" | "ư" | "ươ" | "uô" | "ua" => {
            &["c", "m", "n", "ng", "p", "t"]
        }
        "e" => &["m", "n", "ng"],
        "ê" => &["c", "ch", "m", "n", "ng", "nh", "p", "t"],
        "i" | "iê" => &["ch", "m", "n", "nh", "p", "t"],
        "ơ" => &["m", "n", "ng", "p", "t"],
        "oa" => &["c", "ch", "m", "n", "ng", "nh", "p", "t"],
        "uyê" | "yê" => &["n", "nh", "t", "ch"],
        _ => &[],
    };
    set.contains(&fin)
}

/// Tone forms of a lowercase vowel: [level, sắc, huyền, hỏi, ngã, nặng].
fn tone_forms(base: char) -> [char; 6] {
    match base {
        'a' => ['a', 'á', 'à', 'ả', 'ã', 'ạ'],
        'ă' => ['ă', 'ắ', 'ằ', 'ẳ', 'ẵ', 'ặ'],
        'â' => ['â', 'ấ', 'ầ', 'ẩ', 'ẫ', 'ậ'],
        'e' => ['e', 'é', 'è', 'ẻ', 'ẽ', 'ẹ'],
        'ê' => ['ê', 'ế', 'ề', 'ể', 'ễ', 'ệ'],
        'i' => ['i', 'í', 'ì', 'ỉ', 'ĩ', 'ị'],
        'o' => ['o', 'ó', 'ò', 'ỏ', 'õ', 'ọ'],
        'ô' => ['ô', 'ố', 'ồ', 'ổ', 'ỗ', 'ộ'],
        'ơ' => ['ơ', 'ớ', 'ờ', 'ở', 'ỡ', 'ợ'],
        'u' => ['u', 'ú', 'ù', 'ủ', 'ũ', 'ụ'],
        'ư' => ['ư', 'ứ', 'ừ', 'ử', 'ữ', 'ự'],
        'y' => ['y', 'ý', 'ỳ', 'ỷ', 'ỹ', 'ỵ'],
        other => panic!("no tone forms for {other}"),
    }
}

/// Telex keys for one decorated lowercase vowel.
fn telex_vowel(c: char) -> String {
    match c {
        'ă' => "aw".into(),
        'â' => "aa".into(),
        'ê' => "ee".into(),
        'ô' => "oo".into(),
        'ơ' => "ow".into(),
        'ư' => "uw".into(),
        other => other.to_string(),
    }
}

/// VNI keys for one decorated lowercase vowel.
fn vni_vowel(c: char) -> String {
    match c {
        'ă' => "a8".into(),
        'â' => "a6".into(),
        'ê' => "e6".into(),
        'ô' => "o6".into(),
        'ơ' => "o7".into(),
        'ư' => "u7".into(),
        other => other.to_string(),
    }
}

/// Tone index (0 = level, 1 = sắc, 2 = huyền, 3 = hỏi, 4 = ngã, 5 = nặng) on the vowel that the
/// standard orthography marks.
fn tone_vowel_index(nuc: &[char], has_final: bool, modern: bool) -> usize {
    let s: String = nuc.iter().collect();
    if s.contains("ươ") {
        return nuc.iter().position(|&c| c == 'ơ').unwrap();
    }
    if let Some(p) = nuc.iter().position(|c| "ăâêôơư".contains(*c)) {
        return p;
    }
    match nuc.len() {
        1 => 0,
        2 if has_final => 1,
        2 if matches!(s.as_str(), "oa" | "oe" | "uy") => usize::from(modern),
        2 => 0,
        _ => 1,
    }
}

struct Syllable {
    init: &'static str,
    nuc: Vec<char>,
    fin: &'static str,
    tone: usize,
}

impl Syllable {
    fn expected(&self, modern: bool) -> String {
        let mut nuc = self.nuc.clone();
        if self.tone != 0 {
            let i = tone_vowel_index(&nuc, !self.fin.is_empty(), modern);
            nuc[i] = tone_forms(nuc[i])[self.tone];
        }
        let mut out = self.init.to_string();
        out.extend(nuc);
        out.push_str(self.fin);
        out
    }

    /// Telex keys. `style`: 0 = tone key last, 1 = tone key right after the vowel group,
    /// 2 = 'ươ' typed as 'uo' + one 'w' after the group, tone key last.
    fn telex(&self, style: u8) -> String {
        const TONE: [&str; 6] = ["", "s", "f", "r", "x", "j"];
        let mut out = if self.init == "đ" {
            "dd".to_string()
        } else {
            self.init.to_string()
        };
        let has_uo = self.nuc.windows(2).any(|w| w == ['ư', 'ơ']);
        let mut nucs = String::new();
        if style == 2 && has_uo {
            let mut i = 0;
            while i < self.nuc.len() {
                if self.nuc[i] == 'ư' && self.nuc.get(i + 1) == Some(&'ơ') {
                    nucs.push_str("uo");
                    i += 2;
                } else {
                    nucs.push_str(&telex_vowel(self.nuc[i]));
                    i += 1;
                }
            }
            nucs.push('w');
        } else {
            for &c in &self.nuc {
                nucs.push_str(&telex_vowel(c));
            }
        }
        out.push_str(&nucs);
        if style == 1 {
            out.push_str(TONE[self.tone]);
        }
        out.push_str(self.fin);
        if style != 1 {
            out.push_str(TONE[self.tone]);
        }
        out
    }

    /// VNI keys. `style` as for `telex`, without the horn-trailing variant.
    fn vni(&self, style: u8) -> String {
        const TONE: [&str; 6] = ["", "1", "2", "3", "4", "5"];
        let mut out = if self.init == "đ" {
            "d9".to_string()
        } else {
            self.init.to_string()
        };
        for &c in &self.nuc {
            out.push_str(&vni_vowel(c));
        }
        if style == 1 {
            out.push_str(TONE[self.tone]);
        }
        out.push_str(self.fin);
        if style != 1 {
            out.push_str(TONE[self.tone]);
        }
        out
    }
}

/// Every syllable of the inventory that the orthography rules allow, in a fixed order.
fn syllables() -> Vec<Syllable> {
    let mut out = Vec::new();
    for &init in INITIALS {
        for &nuc_s in NUCLEI {
            let nuc: Vec<char> = nuc_s.chars().collect();
            let f0 = nuc[0];
            let ok_init = match init {
                "c" => !"ieêy".contains(f0),
                "k" => "ieêy".contains(f0),
                "g" | "ng" => !"ieê".contains(f0) && f0 != 'i',
                "gh" | "ngh" => "ieê".contains(f0),
                "gi" => f0 != 'i',
                "qu" => f0 != 'u' && f0 != 'ư',
                _ => true,
            };
            if !ok_init || (nuc_s == "uơ" && !matches!(init, "h" | "kh" | "th" | "qu")) {
                continue;
            }
            for fin in ["", "c", "ch", "m", "n", "ng", "nh", "p", "t"] {
                if fin.is_empty() && NEEDS_FINAL.contains(&nuc_s) {
                    continue;
                }
                if !fin.is_empty() && !closed_ok(nuc_s, fin) {
                    continue;
                }
                let stop = matches!(fin, "c" | "ch" | "p" | "t");
                for tone in 0..6 {
                    if stop && tone != 1 && tone != 5 {
                        continue;
                    }
                    out.push(Syllable {
                        init,
                        nuc: nuc.clone(),
                        fin,
                        tone,
                    });
                }
            }
        }
    }
    out
}

/// English words that the collision list still restores. Their Vietnamese readings
/// (bôm, kên, dể, pổ, í, ò, ...) are not common words, so the test does not expect them to
/// compose.
const NOT_VIETNAMESE: &[&str] = &[
    "boom", "cheer", "deem", "deer", "door", "keen", "loom", "past", "peer", "poor", "post",
    "queer", "seem", "soon", "veer", "is", "of", "or", "was", "her", "here",
];

/// Runs every syllable through one configuration. Returns (checked, failures).
fn run(input_method: InputMethod, modern: bool) -> (usize, Vec<String>) {
    let mut checked = 0;
    let mut failures = Vec::new();
    let list = syllables();
    for (idx, s) in list.iter().enumerate() {
        let styles: &[u8] = match input_method {
            InputMethod::Telex => &[0, 1, 2],
            InputMethod::Vni => &[0, 1],
        };
        for &style in styles {
            let keys = match input_method {
                InputMethod::Telex => s.telex(style),
                InputMethod::Vni => s.vni(style),
            };
            if NOT_VIETNAMESE.contains(&keys.to_lowercase().as_str()) {
                continue;
            }
            let want = s.expected(modern);
            // Lowercase always; every 7th syllable also all-caps; every 5th capitalised.
            let mut cases = vec![(keys.clone(), want.clone())];
            if idx % 7 == 0 {
                cases.push((keys.to_uppercase(), want.to_uppercase()));
            }
            if idx % 5 == 0 {
                let cap = |t: &str| {
                    let mut c = t.chars();
                    let f = c.next().unwrap();
                    f.to_uppercase().collect::<String>() + c.as_str()
                };
                cases.push((cap(&keys), cap(&want)));
            }
            for (k, w) in cases {
                let mut t = Telex::new(Config {
                    modern_orthography: modern,
                    input_method,
                    ..Default::default()
                });
                let got = t.feed(&format!("{k} "));
                checked += 1;
                if got != format!("{w} ") {
                    failures.push(format!("{k:?}: want {w:?}, got {got:?}"));
                }
            }
        }
    }
    (checked, failures)
}

fn assert_clean(input_method: InputMethod, modern: bool) {
    let (checked, failures) = run(input_method, modern);
    assert!(checked > 20_000, "too few cases checked: {checked}");
    assert!(
        failures.is_empty(),
        "{} of {checked} {input_method:?} syllables (modern={modern}) mis-spelled, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(12)]
    );
}

#[test]
fn telex_syllables_traditional() {
    assert_clean(InputMethod::Telex, false);
}

#[test]
fn telex_syllables_modern() {
    assert_clean(InputMethod::Telex, true);
}

#[test]
fn vni_syllables_traditional() {
    assert_clean(InputMethod::Vni, false);
}

#[test]
fn vni_syllables_modern() {
    assert_clean(InputMethod::Vni, true);
}

/// The families fixed in this change, written out.
#[test]
fn known_words_regressions() {
    let telex = [
        ("huouw ", "hươu "),
        ("khuouw ", "khươu "),
        ("huowng ", "hương "),
        ("thuowcs ", "thước "),
        ("thuowr ", "thuở "),
        ("huow ", "huơ "),
        ("khuow ", "khuơ "),
        ("luow ", "luow "),
        ("moon ", "môn "),
        ("noon ", "nôn "),
        ("beer ", "bể "),
        ("boor ", "bổ "),
        ("moor ", "mổ "),
        ("room ", "rôm "),
        ("seen ", "sên "),
        ("teem ", "têm "),
    ];
    let vni = [
        ("hu7o7ng ", "hương "),
        ("thu7o7ng2 ", "thường "),
        ("thu7o71c ", "thước "),
        ("khu7o7ng ", "khương "),
        ("thu7o73 ", "thuở "),
    ];
    for (input, want) in telex {
        assert_eq!(
            Telex::new(Config::default()).feed(input),
            want,
            "Telex {input:?}"
        );
    }
    for (input, want) in vni {
        let cfg = Config {
            input_method: InputMethod::Vni,
            ..Default::default()
        };
        assert_eq!(Telex::new(cfg).feed(input), want, "VNI {input:?}");
    }
}

/// Free tone order: a tone key typed after a consonant but before the vowel.
#[test]
fn free_tone_order_composes_when_the_vowel_arrives() {
    for (input, want) in [
        ("thfi ", "thì "),
        ("tfhi ", "thì "),
        ("THFI ", "THÌ "),
        ("tfi ", "tì "),
        ("ngfi ", "ngì "),
        ("thji ", "thị "),
        ("thxi ", "thĩ "),
        ("tri ", "tri "),
        ("xa ", "xa "),
        ("ra ", "ra "),
        ("sau ", "sau "),
    ] {
        assert_eq!(Telex::new(Config::default()).feed(input), want, "{input:?}");
    }
}

/// With the switch off, the tone key before the vowel stays literal, as before.
#[test]
fn free_tone_order_off_keeps_old_behaviour() {
    let cfg = || Config {
        free_tone_order: false,
        ..Default::default()
    };
    for (input, want) in [
        ("thfi ", "thfi "),
        ("tfhi ", "tfhi "),
        ("tsa ", "tsa "),
        ("sau ", "sau "),
    ] {
        assert_eq!(Telex::new(cfg()).feed(input), want, "{input:?}");
    }
}

/// English that must stay English with the switch on.
#[test]
fn free_tone_order_leaves_english_alone() {
    for word in ["https", "html", "shift", "for", "from", "find", "fi"] {
        assert_eq!(
            Telex::new(Config::default()).feed(&format!("{word} ")),
            format!("{word} "),
            "{word}"
        );
    }
    assert_eq!(Telex::new(Config::default()).feed("this "), "thí ");
}

/// Backspace removes the pending letter one char at a time and drops the pending tone with it.
#[test]
fn backspace_through_pending_tone() {
    let mut t = Telex::new(Config::default());
    assert_eq!(t.feed("thf\u{8}"), "th");
    assert_eq!(t.feed("thfi\u{8}"), "th");
    assert_eq!(t.feed("thfi\u{8}\u{8}\u{8}a "), "a ");
    // Removing the pending letter itself: the next vowel is a plain vowel again.
    assert_eq!(t.feed("thf\u{8}i "), "thi ");
    // The pending f survives: only the h went, so the tone still applies.
    assert_eq!(t.feed("tfh\u{8}i "), "tì ");
}

/// Randomised screen consistency with pending tones, Backspace, boundaries and uppercase.
/// The screen is rebuilt from the handled/unhandled results of key() and backspace() only.
#[test]
fn pending_tone_screen_stays_consistent() {
    let alphabet: Vec<char> = "thfsrxjiaeoyTHFIxn \u{8}".chars().collect();
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..3000 {
        let mut t = Telex::new(Config::default());
        let mut screen: Vec<char> = Vec::new();
        for _ in 0..40 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let ch = alphabet[(seed >> 33) as usize % alphabet.len()];
            let r = if ch == '\u{8}' {
                t.backspace()
            } else {
                t.key(ch)
            };
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
            let disp: Vec<char> = t.displayed().chars().collect();
            assert!(
                screen.ends_with(&disp),
                "screen {screen:?} vs composing {disp:?}"
            );
        }
    }
}

fn with(simple: bool, quick: bool) -> Config {
    Config {
        simple_telex: simple,
        quick_telex: quick,
        ..Default::default()
    }
}

/// simple_telex: 'w' with nothing to horn or breve is a plain letter.
#[test]
fn simple_telex_on_and_off() {
    for (input, on, off) in [
        ("was ", "was ", "was "),
        ("win ", "win ", "win "),
        ("w ", "w ", "ư "),
        ("tw ", "tw ", "tư "),
        // 'w' still modifies an existing vowel, with or without the option.
        ("aw ", "ă ", "ă "),
        ("ow ", "ơ ", "ơ "),
        ("uw ", "ư ", "ư "),
        ("tuw ", "tư ", "tư "),
        ("nguowif ", "người ", "người "),
        ("Ww ", "Ww ", "W "),
    ] {
        assert_eq!(
            Telex::new(with(true, false)).feed(input),
            on,
            "on {input:?}"
        );
        assert_eq!(
            Telex::new(with(false, false)).feed(input),
            off,
            "off {input:?}"
        );
    }
}

/// quick_telex: the seven OpenKey pairs, only at the start of a word.
#[test]
fn quick_telex_pairs_on_and_off() {
    for (input, on, off) in [
        ("cca ", "cha ", "cca "),
        ("gga ", "gia ", "gga "),
        ("kka ", "kha ", "kka "),
        ("nna ", "nga ", "nna "),
        ("ppo ", "pho ", "ppo "),
        ("qqa ", "qua ", "qqa "),
        ("tto ", "tho ", "tto "),
        ("ccoong ", "chông ", "ccoong "),
        ("ttoo ", "thô ", "ttoo "),
        // Uppercase: the second letter takes the case of the typed key.
        ("Ttoo ", "Thô ", "Ttoo "),
        ("TTOO ", "THÔ ", "TTOO "),
        ("TToo ", "THô ", "TToo "),
        ("TTo ", "THo ", "TTo "),
        ("tToo ", "thô ", "tToo "),
        // A new word after a space starts again; a doubled pair in the middle of a word is unchanged.
        ("ban nna ", "ban nga ", "ban nna "),
        ("ban nan ", "ban nan ", "ban nan "),
        ("bnn ", "bnn ", "bnn "),
        ("ccc ", "ccc ", "ccc "),
    ] {
        assert_eq!(
            Telex::new(with(false, true)).feed(input),
            on,
            "on {input:?}"
        );
        assert_eq!(
            Telex::new(with(false, false)).feed(input),
            off,
            "off {input:?}"
        );
    }
}

/// Backspace after an expansion leaves the screen and the composition in step.
#[test]
fn quick_telex_backspace_is_consistent() {
    let cfg = || with(false, true);
    assert_eq!(Telex::new(cfg()).feed("cc\u{8}a "), "ca ");
    assert_eq!(Telex::new(cfg()).feed("cca\u{8}\u{8}"), "c");
    assert_eq!(Telex::new(cfg()).feed("TTo\u{8}"), "TH");
    assert_eq!(Telex::new(cfg()).feed("ttoo\u{8}\u{8}"), "t");
    assert_eq!(Telex::new(cfg()).feed("ttoo\u{8}\u{8}\u{8}a "), "a ");
}

/// Both options on together, and with the default config: identical to the pre-option output.
#[test]
fn default_config_is_unchanged_by_the_new_options() {
    for input in [
        "was ",
        "w ",
        "ww ",
        "cca ",
        "ttoo ",
        "nguowif ",
        "Vieetj Nam ",
        "tfi ",
    ] {
        assert_eq!(
            Telex::new(Config::default()).feed(input),
            Telex::new(with(false, false)).feed(input),
            "{input:?}"
        );
    }
    assert!(!Config::default().simple_telex);
    assert!(!Config::default().quick_telex);
}

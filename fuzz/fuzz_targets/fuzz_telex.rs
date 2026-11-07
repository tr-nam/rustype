#![no_main]
use libfuzzer_sys::fuzz_target;
use rustype_core::telex::{Config, Telex};

fuzz_target!(|data: &str| {
    let mut t = Telex::new(Config::default());
    let _ = t.feed(data);

    // Also fuzz with modern orthography.
    let mut t2 = Telex::new(Config { modern_orthography: true, auto_restore_english: true });
    let _ = t2.feed(data);

    // Also fuzz with auto_restore_english off.
    let mut t3 = Telex::new(Config { modern_orthography: false, auto_restore_english: false });
    let _ = t3.feed(data);
});

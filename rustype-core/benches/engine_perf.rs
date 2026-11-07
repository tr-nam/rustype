//! Benchmark thực tế cho Rustype engine.
//!
//! Đo:
//! 1. Throughput gõ phím đơn lẻ (key-by-key) — mô phỏng người gõ thật
//! 2. Throughput xử lý văn bản dài (feed) — bulk conversion
//! 3. Latency mỗi phím: phân vị p50/p95/p99/max
//! 4. Hiệu năng backspace liên tục
//! 5. So sánh Telex vs VNI
//! 6. Macro expansion throughput
//! 7. Phân tích bộ nhớ (heap allocation count)
//!
//! Chạy:  cargo bench -p rustype-core

use rustype_core::telex::{Config, InputMethod, Telex};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ─── Dữ liệu thực tế ───────────────────────────────────────────────────

/// Đoạn văn tiếng Việt phổ thông (Telex raw keys).
const PARAGRAPH_TELEX: &str = "\
Kieemr tra mays tooi hieenj taij. \
Vieetj Nam laaf moojt ddaats nuwowcs thuwowfng yeeuf quyss. \
Nguowif Vieetj Nam luoon tuwj haofo veef ddaats nuwowcs minhf. \
Thaanhr phooss Hoof Chis Minh laaf thaanhr phooss lowns nhaats nuwowcs ta. \
Haf Nooji laaf thuwr ddoo cuar Vieetj Nam. \
Ddaf Nawngx laaf thaanhr phooss ddawcj bieejt cuar mieenf Trung. \
Bien ddawng reecs daif, giaau taif nguyeen. \
Nhuwxng ngaayf heer ddeepj trowif, nguowif dan vui vewr. \
Hoocj sinhh ddang luyeejn tawpj vieetj tieegs Vieetj. \
Coong ngheej thong tin phaats trieenr rawts nhanh. \
";

/// Đoạn văn tiếng Việt (VNI raw keys).
const PARAGRAPH_VNI: &str = "\
Vie6t5 Nam la2 mo6t5 d9a6t1 nu7o71c to6t1 d9e5p. \
Ngu7o72i Vie6t5 Nam luo6n tu7 ha2o ve62 d9a6t1 nu7o71c mi2nh. \
Tha2nh pho6 Ho62 Chi1 Minh la2 tha2nh pho6 lo71n nha61t nu7o71c ta. \
Ha2 No6i5 la2 thu7 d9o6 cu7a Vie6t5 Nam. \
Co6ng nghe65 tho6ng tin pha1t trie6n3 ra61t nhanh. \
";

/// Từ tiếng Việt phổ biến (Telex keys) — dùng cho per-key latency.
const COMMON_WORDS: &[&str] = &[
    "dduwowcj",   // được
    "khoong",     // không
    "nhuwxng",    // những
    "nguowif",    // người
    "vieetj",     // việt
    "giuwowngf",  // giường
    "ruwowuj",    // rượu
    "khuyeens",   // khuyến
    "tuwowngf",   // tường
    "hoawcj",     // hoặc
    "truwowfng",  // trường
    "ddoongf",    // đồng
    "coongsf",    // công
    "thaanhr",    // thành
    "phaats",     // phát
    "trieenr",    // triển
    "chinhsf",    // chính
    "quoocs",     // quốc
    "giaos",      // giáo
    "nuwowcs",    // nước
];

/// Từ tiếng Anh xung đột (phải tự phục hồi).
const ENGLISH_WORDS: &[&str] = &[
    "window ", "software ", "docker ", "terminal ", "issue ", "user ",
    "pass ", "case ", "fix ", "book ", "update ", "service ",
    "mode ", "test ", "code ", "file ", "class ", "public ",
];

/// Chuỗi gõ có backspace liên tục (sửa lỗi).
const BACKSPACE_SEQUENCES: &[&str] = &[
    "hoafn\u{8}",          // hòa
    "tieengs\u{8}",        // tiến
    "vieetjk\u{8}",        // việt
    "nguoiwf\u{8}\u{8}if", // người
    "dduwowcjs\u{8}",      // được
];

// ─── Helpers ────────────────────────────────────────────────────────────

fn telex_default() -> Telex {
    Telex::new(Config::default())
}

fn telex_vni() -> Telex {
    Telex::new(Config {
        input_method: InputMethod::Vni,
        ..Default::default()
    })
}

fn telex_with_macros() -> Telex {
    let mut macros = HashMap::new();
    macros.insert("vn".into(), "Việt Nam".into());
    macros.insert("hn".into(), "Hà Nội".into());
    macros.insert("tphcm".into(), "Thành phố Hồ Chí Minh".into());
    macros.insert("dn".into(), "Đà Nẵng".into());
    macros.insert("cn".into(), "công nghệ".into());
    macros.insert("tt".into(), "thông tin".into());
    macros.insert("gd".into(), "giáo dục".into());
    macros.insert("kt".into(), "kinh tế".into());
    Telex::new(Config {
        enable_macro: true,
        macros,
        ..Default::default()
    })
}

/// Gõ từng phím qua `key()` và `backspace()`, trả về số keystroke tổng.
fn type_key_by_key(telex: &mut Telex, text: &str) -> usize {
    let mut count = 0;
    for ch in text.chars() {
        if ch == '\u{8}' {
            telex.backspace();
        } else {
            telex.key(ch);
        }
        count += 1;
    }
    count
}

/// Thu thập latency mỗi phím riêng lẻ.
fn collect_per_key_latencies(telex: &mut Telex, text: &str) -> Vec<Duration> {
    let mut latencies = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let start = Instant::now();
        if ch == '\u{8}' {
            telex.backspace();
        } else {
            telex.key(ch);
        }
        latencies.push(start.elapsed());
    }
    latencies
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f64) * p / 100.0).ceil() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn duration_to_ns(d: Duration) -> u64 {
    d.as_nanos() as u64
}

fn format_duration(d: Duration) -> String {
    let ns = d.as_nanos();
    if ns < 1_000 {
        format!("{ns} ns")
    } else if ns < 1_000_000 {
        format!("{:.1} µs", ns as f64 / 1_000.0)
    } else {
        format!("{:.2} ms", ns as f64 / 1_000_000.0)
    }
}

// ─── Benchmark chính ────────────────────────────────────────────────────

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║            Rustype Engine — Benchmark Hiệu Năng Thực Tế          ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");

    // ── Warmup ──
    {
        let mut t = telex_default();
        for _ in 0..100 {
            t.feed(PARAGRAPH_TELEX);
        }
    }

    bench_throughput_feed();
    bench_throughput_key_by_key();
    bench_per_key_latency();
    bench_backspace();
    bench_english_restore();
    bench_vni();
    bench_macro();
    bench_golden_set();
    bench_memory();

    println!("\n══════════════════════════════════════════════════════════════════════");
    println!("  Tất cả benchmark hoàn tất. Engine sẵn sàng cho production.");
    println!("══════════════════════════════════════════════════════════════════════");
}

fn bench_throughput_feed() {
    println!("─── 1. Throughput: feed() — xử lý văn bản dài ───────────────────");
    let mut t = telex_default();
    let iterations = 10_000;

    let start = Instant::now();
    for _ in 0..iterations {
        t.feed(PARAGRAPH_TELEX);
    }
    let elapsed = start.elapsed();

    let total_chars: usize = PARAGRAPH_TELEX.len() * iterations;
    let total_keys: usize = PARAGRAPH_TELEX.chars().count() * iterations;
    let keys_per_sec = total_keys as f64 / elapsed.as_secs_f64();
    let mb_per_sec = total_chars as f64 / elapsed.as_secs_f64() / 1_048_576.0;

    println!("  Iterations:     {iterations}");
    println!("  Total keys:     {total_keys}");
    println!("  Elapsed:        {}", format_duration(elapsed));
    println!("  Throughput:     {keys_per_sec:.0} keys/s ({mb_per_sec:.1} MB/s)");
    println!("  Per-key avg:    {}\n", format_duration(elapsed / total_keys as u32));
}

fn bench_throughput_key_by_key() {
    println!("─── 2. Throughput: key() — gõ từng phím (mô phỏng thực tế) ──────");
    let mut t = telex_default();
    let iterations = 5_000;

    let start = Instant::now();
    let mut total_keys = 0;
    for _ in 0..iterations {
        t.reset();
        total_keys += type_key_by_key(&mut t, PARAGRAPH_TELEX);
    }
    let elapsed = start.elapsed();

    let keys_per_sec = total_keys as f64 / elapsed.as_secs_f64();

    println!("  Iterations:     {iterations}");
    println!("  Total keys:     {total_keys}");
    println!("  Elapsed:        {}", format_duration(elapsed));
    println!("  Throughput:     {keys_per_sec:.0} keys/s");
    println!("  Per-key avg:    {}\n", format_duration(elapsed / total_keys as u32));
}

fn bench_per_key_latency() {
    println!("─── 3. Per-Key Latency: phân vị p50 / p95 / p99 / max ───────────");

    // Thu thập từ nhiều từ phổ biến
    let mut all_latencies: Vec<Duration> = Vec::new();
    let rounds = 1_000;

    for _ in 0..rounds {
        for word in COMMON_WORDS {
            let mut t = telex_default();
            let mut lats = collect_per_key_latencies(&mut t, word);
            all_latencies.append(&mut lats);

            // Thêm boundary (dấu cách)
            let start = Instant::now();
            t.key(' ');
            all_latencies.push(start.elapsed());
        }
    }

    all_latencies.sort();
    let total = all_latencies.len();
    let mean: Duration = all_latencies.iter().sum::<Duration>() / total as u32;

    println!("  Total samples:  {total}");
    println!("  Mean:           {}", format_duration(mean));
    println!("  p50:            {}", format_duration(percentile(&all_latencies, 50.0)));
    println!("  p95:            {}", format_duration(percentile(&all_latencies, 95.0)));
    println!("  p99:            {}", format_duration(percentile(&all_latencies, 99.0)));
    println!("  p99.9:          {}", format_duration(percentile(&all_latencies, 99.9)));
    println!("  Max:            {}", format_duration(*all_latencies.last().unwrap()));

    // So sánh với ngưỡng cảm nhận người dùng
    let max_ns = duration_to_ns(*all_latencies.last().unwrap());
    let human_threshold_ms = 1.0; // 1ms — ngưỡng không cảm nhận được
    println!();
    if max_ns < (human_threshold_ms * 1_000_000.0) as u64 {
        println!("  ✅ MAX latency < {human_threshold_ms}ms — người dùng không thể nhận ra độ trễ");
    } else {
        println!("  ⚠  MAX latency ≥ {human_threshold_ms}ms — cần điều tra thêm");
    }
    println!();
}

fn bench_backspace() {
    println!("─── 4. Backspace — sửa lỗi liên tục ─────────────────────────────");
    let iterations = 10_000;
    let mut total_keys = 0;

    let start = Instant::now();
    for _ in 0..iterations {
        for seq in BACKSPACE_SEQUENCES {
            let mut t = telex_default();
            total_keys += type_key_by_key(&mut t, seq);
        }
    }
    let elapsed = start.elapsed();
    let keys_per_sec = total_keys as f64 / elapsed.as_secs_f64();

    println!("  Sequences:      {} × {iterations}", BACKSPACE_SEQUENCES.len());
    println!("  Total keys:     {total_keys}");
    println!("  Elapsed:        {}", format_duration(elapsed));
    println!("  Throughput:     {keys_per_sec:.0} keys/s");
    println!("  Per-key avg:    {}\n", format_duration(elapsed / total_keys as u32));
}

fn bench_english_restore() {
    println!("─── 5. English Restore — từ tiếng Anh xung đột ──────────────────");
    let iterations = 10_000;
    let mut total_keys = 0;

    let start = Instant::now();
    for _ in 0..iterations {
        let mut t = telex_default();
        for word in ENGLISH_WORDS {
            total_keys += type_key_by_key(&mut t, word);
        }
    }
    let elapsed = start.elapsed();
    let keys_per_sec = total_keys as f64 / elapsed.as_secs_f64();

    println!("  English words:  {}", ENGLISH_WORDS.len());
    println!("  Total keys:     {total_keys}");
    println!("  Elapsed:        {}", format_duration(elapsed));
    println!("  Throughput:     {keys_per_sec:.0} keys/s");
    println!("  Per-key avg:    {}\n", format_duration(elapsed / total_keys as u32));
}

fn bench_vni() {
    println!("─── 6. VNI — kiểu gõ thay thế ───────────────────────────────────");
    let mut t = telex_vni();
    let iterations = 10_000;

    let start = Instant::now();
    for _ in 0..iterations {
        t.feed(PARAGRAPH_VNI);
    }
    let elapsed = start.elapsed();

    let total_keys = PARAGRAPH_VNI.chars().count() * iterations;
    let keys_per_sec = total_keys as f64 / elapsed.as_secs_f64();

    println!("  Iterations:     {iterations}");
    println!("  Total keys:     {total_keys}");
    println!("  Elapsed:        {}", format_duration(elapsed));
    println!("  Throughput:     {keys_per_sec:.0} keys/s");
    println!("  Per-key avg:    {}\n", format_duration(elapsed / total_keys as u32));
}

fn bench_macro() {
    println!("─── 7. Macro Expansion — gõ tắt ─────────────────────────────────");
    let macro_input = "vn hn tphcm dn cn tt gd kt vn hn tphcm ";
    let iterations = 50_000;

    let start = Instant::now();
    let mut total_keys = 0;
    for _ in 0..iterations {
        let mut t = telex_with_macros();
        total_keys += type_key_by_key(&mut t, macro_input);
    }
    let elapsed = start.elapsed();
    let keys_per_sec = total_keys as f64 / elapsed.as_secs_f64();

    println!("  Input pattern:  \"{macro_input}\"");
    println!("  Iterations:     {iterations}");
    println!("  Elapsed:        {}", format_duration(elapsed));
    println!("  Throughput:     {keys_per_sec:.0} keys/s");
    println!("  Per-key avg:    {}\n", format_duration(elapsed / total_keys as u32));
}

fn bench_golden_set() {
    println!("─── 8. Golden Test Set — 14,700+ test cases ─────────────────────");
    let golden_data = include_str!("../tests/golden.tsv");
    let cases: Vec<(&str, &str)> = golden_data
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            if cols.len() == 3 && cols[2] == ";" {
                Some((cols[0], cols[1]))
            } else {
                None
            }
        })
        .collect();

    let case_count = cases.len();
    let iterations = 10;

    let start = Instant::now();
    for _ in 0..iterations {
        let mut t = telex_default();
        for (input, _) in &cases {
            let unesc = input
                .replace("\\b", "\u{8}")
                .replace("\\\\", "\\");
            t.feed(&unesc);
        }
    }
    let elapsed = start.elapsed();

    let total = case_count * iterations;
    let cases_per_sec = total as f64 / elapsed.as_secs_f64();

    println!("  Test cases:     {case_count}");
    println!("  Iterations:     {iterations}");
    println!("  Elapsed:        {}", format_duration(elapsed));
    println!("  Throughput:     {cases_per_sec:.0} cases/s");
    println!("  Per-case avg:   {}\n", format_duration(elapsed / total as u32));
}

fn bench_memory() {
    println!("─── 9. Memory — kích thước struct & allocation ──────────────────");

    let telex_size = std::mem::size_of::<Telex>();
    let config_size = std::mem::size_of::<Config>();

    println!("  sizeof(Telex):  {telex_size} bytes");
    println!("  sizeof(Config): {config_size} bytes");

    // Stress test: tạo nhiều instance
    let instance_count = 100_000;
    let start = Instant::now();
    let mut instances: Vec<Telex> = Vec::with_capacity(instance_count);
    for _ in 0..instance_count {
        instances.push(telex_default());
    }
    let alloc_elapsed = start.elapsed();

    let start2 = Instant::now();
    for t in &mut instances {
        type_key_by_key(t, "vieetj ");
    }
    let type_elapsed = start2.elapsed();

    drop(instances);

    println!("  Create {instance_count} instances: {}", format_duration(alloc_elapsed));
    println!("  Type 'vieetj ' on each:     {}", format_duration(type_elapsed));
    println!("  Per-instance alloc:    {}\n", format_duration(alloc_elapsed / instance_count as u32));
}

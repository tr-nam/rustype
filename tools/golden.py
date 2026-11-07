"""Golden cases for the Telex core: tests/golden.tsv, checked by `cargo test`.

  python3 -I tools/golden.py diff          # show cases whose output changed
  python3 -I tools/golden.py update        # accept the current outputs
  python3 -I tools/golden.py update --new  # build a fresh set of inputs, then record outputs

Run `cargo build --release` first: outputs come from target/release/ibus-engine-rustype --feed.
"""
import pathlib, random, re, subprocess, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "ibus-engine-rustype"
GOLDEN = ROOT / "rustype-core" / "tests" / "golden.tsv"
HEADER = """\
# Kết quả chuẩn của lõi Telex, được `cargo test` kiểm tra.
# Mỗi dòng: phím gõ <TAB> chữ trên màn hình <TAB> ;
# \\b là phím BackSpace. Dấu ; ở cuối giữ lại khoảng trắng cuối chữ.
# Không sửa tay: dùng `python3 -I tools/golden.py update` sau khi cố ý đổi luật gõ.
"""

SENTENCES = [
    "Kieemr tra mays tooi hieenj taij toanf booj gox vaof terminal cuar banj ddang bij looix",
    "Vaf mays ddang cos hai Vsc caanf xoas 1 cais",
    "Neeus taws cheees ddooj gachj chaan thif ddang gox maf nhaans chuootj sang chox khacs laf maats luoon chuwx ddos",
    "nguowif Vieetj Nam yeeu nuwowcs, hoaf binhf, khoer manhj, thuyr chung",
    "giuwowngf, khuyeens, ruwowuj, hoawcj, quas, gias, muwa, cuwar, dduwowcj",
    "VIEETJ NAM, Vieetj Nam, vieetj nam",
    "aa aaa ss dd ddd ww w uw ow aw",
    "hoafn\b\b tieengs\b khoong\b\b\b",
]
MARK = {"â": "aa", "ă": "aw", "ê": "ee", "ô": "oo", "ơ": "ow", "ư": "uw", "đ": "dd"}


def rust_list(name):
    src = (ROOT / "rustype-core" / "src" / "tables.rs").read_text(encoding="utf-8")
    m = re.search(rf"pub const {name}: &\[&str\] = &\[(.*?)\];", src, re.S)
    return re.findall(r'"([^"]*)"', m.group(1))


def keys(s):
    return "".join(MARK.get(c, c) for c in s)


def new_inputs():
    rng = random.Random(20261107)
    syllables = set()
    for i in rust_list("VALID_INITIALS"):
        for n in rust_list("NUCLEI_COMMITTED"):
            for f in rust_list("VALID_FINALS"):
                for t in ["", "s", "f", "r", "x", "j"]:
                    syllables.add(keys(i + n + f) + t)            # tone at the end
                    syllables.add(keys(i + n) + t + keys(f))      # tone right after the vowels
                    if "ươ" in n:
                        syllables.add(keys(i + n.replace("ươ", "uow") + f) + t)
    cases = list(SENTENCES)
    cases += [s + " " for s in rng.sample(sorted(syllables), 8000)]
    words = []
    dict_file = pathlib.Path("/usr/share/dict/american-english")
    if dict_file.exists():
        words = sorted({w.strip() for w in dict_file.read_text(encoding="utf-8").split()
                        if w.strip().isascii() and w.strip().isalpha() and len(w.strip()) <= 14})
    picked = rng.sample(words, min(3000, len(words)))
    cases += [w + " " for w in picked] + [w.capitalize() + "." for w in picked[:500]]
    cases += [w + " " for w in rust_list("ENGLISH_COLLISION_WORDS")]
    alphabet = "aeiouydwsfrxjzqgnhtcmpbklv" * 3 + "AEOUWDS" + " ,.\b\b"
    cases += ["".join(rng.choice(alphabet) for _ in range(rng.randint(1, 18))) for _ in range(3000)]
    seen, out = set(), []
    for c in cases:
        if c not in seen:
            seen.add(c)
            out.append(c)
    return out


def esc(s):
    return s.replace("\\", "\\\\").replace("\b", "\\b")


def unesc(s):
    return s.replace("\\b", "\b").replace("\\\\", "\\")


def load():
    rows = []
    for line in GOLDEN.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        inp, out, end = line.split("\t")
        assert end == ";", line
        rows.append((unesc(inp), unesc(out)))
    return rows


def run(inputs):
    if not BIN.exists():
        sys.exit("build first: cargo build --release")
    p = subprocess.run([str(BIN), "--feed"], input="\n".join(inputs) + "\n", capture_output=True, text=True, check=True)
    out = p.stdout.split("\n")[:-1]
    assert len(out) == len(inputs)
    return out


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "diff"
    if cmd == "update":
        inputs = new_inputs() if ("--new" in sys.argv or not GOLDEN.exists()) else [i for i, _ in load()]
        outputs = run(inputs)
        GOLDEN.parent.mkdir(exist_ok=True)
        with GOLDEN.open("w", encoding="utf-8") as f:
            f.write(HEADER)
            for i, o in zip(inputs, outputs):
                f.write(f"{esc(i)}\t{esc(o)}\t;\n")
        print(f"wrote {len(inputs)} cases to {GOLDEN.relative_to(ROOT)}")
    elif cmd == "diff":
        rows = load()
        got = run([i for i, _ in rows])
        changed = [(i, w, g) for (i, w), g in zip(rows, got) if w != g]
        for i, w, g in changed[:50]:
            print(f"{i!r}\n  before: {w!r}\n  now:    {g!r}")
        print(f"{len(changed)} of {len(rows)} cases changed")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()

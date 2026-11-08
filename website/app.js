/**
 * Rustype Online Typing Demo Engine
 * Emulates the pure Telex & VNI composition logic in JavaScript for the web demo.
 */

(function () {
  const textarea = document.getElementById("demo-textarea");
  const clearBtn = document.getElementById("btn-clear-demo");
  const radioButtons = document.querySelectorAll('input[name="demo-method"]');

  let currentMethod = "telex";

  radioButtons.forEach((radio) => {
    radio.addEventListener("change", (e) => {
      currentMethod = e.target.value;
      textarea.focus();
    });
  });

  clearBtn.addEventListener("click", () => {
    textarea.value = "";
    textarea.focus();
  });

  window.insertHint = function (text) {
    textarea.value += text;
    textarea.focus();
    textarea.scrollTop = textarea.scrollHeight;
  };

  // Vowel tone tables
  const TONES = {
    // a
    "a": ["a", "á", "à", "ả", "ã", "ạ"],
    "ă": ["ă", "ắ", "ằ", "ẳ", "ẵ", "ặ"],
    "â": ["â", "ấ", "ầ", "ẩ", "ẫ", "ậ"],
    // e
    "e": ["e", "é", "è", "ẻ", "ẽ", "ẹ"],
    "ê": ["ê", "ế", "ề", "ể", "ễ", "ệ"],
    // i
    "i": ["i", "í", "ì", "ỉ", "ĩ", "ị"],
    // o
    "o": ["o", "ó", "ò", "ỏ", "õ", "ọ"],
    "ô": ["ô", "ố", "ồ", "ổ", "ỗ", "ộ"],
    "ơ": ["ơ", "ớ", "ờ", "ở", "ỡ", "ợ"],
    // u
    "u": ["u", "ú", "ù", "ủ", "ũ", "ụ"],
    "ư": ["ư", "ứ", "ừ", "ử", "ữ", "ự"],
    // y
    "y": ["y", "ý", "ỳ", "ỷ", "ỹ", "ỵ"]
  };

  function removeTone(ch) {
    for (const [base, variants] of Object.entries(TONES)) {
      const idx = variants.indexOf(ch);
      if (idx !== -1) {
        return { base, tone: idx };
      }
    }
    return { base: ch, tone: 0 };
  }

  function applyTone(ch, toneIdx) {
    const { base } = removeTone(ch);
    if (TONES[base] && toneIdx < TONES[base].length) {
      return TONES[base][toneIdx];
    }
    return ch;
  }

  // Basic in-browser Telex/VNI transformer on input
  textarea.addEventListener("input", (e) => {
    if (e.inputType === "deleteContentBackward") return;

    let text = textarea.value;
    if (!text) return;

    // Find the last typed word
    const lastSpaceIdx = Math.max(text.lastIndexOf(" "), text.lastIndexOf("\n"), text.lastIndexOf("\t"));
    const wordStart = lastSpaceIdx === -1 ? 0 : lastSpaceIdx + 1;
    const word = text.slice(wordStart);

    if (!word || word.length < 2) return;

    let converted = processWord(word, currentMethod);
    if (converted !== word) {
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      const diff = converted.length - word.length;

      textarea.value = text.slice(0, wordStart) + converted;
      textarea.setSelectionRange(start + diff, end + diff);
    }
  });

  function processWord(word, method) {
    let lower = word.toLowerCase();

    // Auto-restore common English words
    const englishWords = ["window", "software", "user", "this", "test", "system", "rust"];
    if (englishWords.includes(lower)) {
      return word;
    }

    if (method === "telex") {
      return convertTelex(word);
    } else {
      return convertVni(word);
    }
  }

  function convertTelex(w) {
    let res = w;
    // Horn & Breve & Circumflex
    res = res.replace(/aw/gi, (m) => m[0] === 'A' ? 'Ă' : 'ă');
    res = res.replace(/aa/gi, (m) => m[0] === 'A' ? 'Â' : 'â');
    res = res.replace(/ee/gi, (m) => m[0] === 'E' ? 'Ê' : 'ê');
    res = res.replace(/oo/gi, (m) => m[0] === 'O' ? 'Ô' : 'ô');
    res = res.replace(/ow/gi, (m) => m[0] === 'O' ? 'Ơ' : 'ơ');
    res = res.replace(/uuw/gi, (m) => m[0] === 'U' ? 'Ưu' : 'ưu'); // Luuw -> Lưu
    res = res.replace(/uw/gi, (m) => m[0] === 'U' ? 'Ư' : 'ư');
    res = res.replace(/dd/gi, (m) => m[0] === 'D' ? 'Đ' : 'đ');
    res = res.replace(/(^|[^a-zA-Z])w/g, '$1ư');
    res = res.replace(/w$/gi, (m) => m === 'W' ? 'Ư' : 'ư');

    // Tones: s=1, f=2, r=3, x=4, j=5
    const toneMap = { s: 1, f: 2, r: 3, x: 4, j: 5 };
    const lastChar = res[res.length - 1]?.toLowerCase();

    if (toneMap[lastChar]) {
      const tone = toneMap[lastChar];
      let withoutKey = res.slice(0, -1);
      let toned = addToneToWord(withoutKey, tone);
      if (toned !== withoutKey) {
        return toned;
      }
    }

    return res;
  }

  function convertVni(w) {
    let res = w;
    // VNI diacritics: 6=circumflex, 7=horn, 8=breve, 9=d stroke
    res = res.replace(/a6/gi, (m) => m[0] === 'A' ? 'Â' : 'â');
    res = res.replace(/e6/gi, (m) => m[0] === 'E' ? 'Ê' : 'ê');
    res = res.replace(/o6/gi, (m) => m[0] === 'O' ? 'Ô' : 'ô');
    res = res.replace(/o7/gi, (m) => m[0] === 'O' ? 'Ơ' : 'ơ');
    res = res.replace(/u7/gi, (m) => m[0] === 'U' ? 'Ư' : 'ư');
    res = res.replace(/a8/gi, (m) => m[0] === 'A' ? 'Ă' : 'ă');
    res = res.replace(/d9/gi, (m) => m[0] === 'D' ? 'Đ' : 'đ');

    // Tones: 1=sắc, 2=huyền, 3=hỏi, 4=ngã, 5=nặng
    const vniToneMap = { "1": 1, "2": 2, "3": 3, "4": 4, "5": 5 };
    const lastChar = res[res.length - 1];

    if (vniToneMap[lastChar]) {
      const tone = vniToneMap[lastChar];
      let withoutKey = res.slice(0, -1);
      let toned = addToneToWord(withoutKey, tone);
      if (toned !== withoutKey) {
        return toned;
      }
    }

    return res;
  }

  function addToneToWord(word, toneIdx) {
    // Find vowel position to place tone
    const chars = [...word];
    let targetIdx = -1;

    // Prefer accented vowels like ê, ô, ơ, ư, â, ă
    for (let i = chars.length - 1; i >= 0; i--) {
      const c = chars[i].toLowerCase();
      if ("êôơưâă".includes(c)) {
        targetIdx = i;
        break;
      }
    }

    // Otherwise find first or last regular vowel
    if (targetIdx === -1) {
      for (let i = chars.length - 1; i >= 0; i--) {
        const c = chars[i].toLowerCase();
        if ("aeiouy".includes(c)) {
          targetIdx = i;
          break;
        }
      }
    }

    if (targetIdx !== -1) {
      const original = chars[targetIdx];
      const isUpper = original === original.toUpperCase() && original !== original.toLowerCase();
      let toned = applyTone(original.toLowerCase(), toneIdx);
      chars[targetIdx] = isUpper ? toned.toUpperCase() : toned;
      return chars.join("");
    }

    return word;
  }
})();

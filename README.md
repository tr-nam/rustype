<p align="center">
  <img src="assets/banner.png" alt="Rustype - Gõ tiếng Việt cùng Rust" width="700">
</p>

<p align="center">
  <strong>Bộ gõ tiếng Việt siêu nhẹ, lõi viết bằng Rust, cho Linux (GNOME trên Wayland), Windows và macOS.</strong>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/language-Rust-orange.svg?style=flat-square&logo=rust" alt="Rust"></a>
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-blue.svg?style=flat-square" alt="Platform">
  <a href="https://github.com/tr-nam/rustype/actions/workflows/ci.yml"><img src="https://github.com/tr-nam/rustype/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/tr-nam/rustype/releases"><img src="https://img.shields.io/badge/release-v0.2.1-blue.svg?style=flat-square" alt="Latest Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="License"></a>
</p>

---

Rustype là bộ gõ tiếng Việt hỗ trợ kiểu gõ Telex & VNI. Lõi gõ và tiến trình trên Linux viết bằng Rust, chỉ dùng thư viện chuẩn. Ứng dụng Cài đặt viết bằng Python với GTK4, lớp vỏ macOS bằng Objective-C, và extension GNOME bằng JavaScript.

Trên từng nền tảng:
- **Linux (GNOME Shell trên Wayland)**: chạy như một tiến trình duy nhất, `rustype-linux`, thay cho `ibus-daemon` và engine IBus cũ. Giải quyết lỗi lặp chữ trong terminal (*Ptyxis, GNOME Terminal, kitty, Alacritty, WezTerm*), không nháy màn hình, bấm chuột không mất chữ. Chỉ hỗ trợ GNOME; xem [COMPATIBILITY.md](docs/COMPATIBILITY.md).
- **Windows**: chạy độc lập qua Low-Level Keyboard Hook (`SetWindowsHookExW` + `SendInput`) và khay hệ thống (System Tray).
- **macOS**: đóng gói theo InputMethodKit (`Rustype.app`), chạy trên Apple Silicon và Intel.

Trải nghiệm gõ thử trực tuyến tại trang chủ: **[rustype.namtr.dev](https://rustype.namtr.dev)**

---

## ⚡ Hiệu năng

Đo bằng `cargo bench -p rustype-core` trên profile `release`, máy phát triển:

| Chỉ số | Kết quả | Ý nghĩa |
|---|---|---|
| **Độ trễ trung vị (p50), một phím** | **461 ns** | Lõi gõ xử lý mỗi phím dưới 1 micro giây |
| **Độ trễ p99** | **892 ns** | 99% số phím được xử lý trong khoảng dưới 1 micro giây |
| **Độ trễ tối đa** | **20,5 µs** | Thấp hơn nhiều so với ngưỡng người dùng cảm nhận được (1 ms) |
| **Throughput gõ từng phím** | **~2.520.000 phím/giây** | Nhanh hơn rất nhiều so với tốc độ gõ của người (~15 phím/giây) |
| **Kích thước struct lõi** | **200 byte** (`Telex`) | Bộ nhớ tiêu thụ thấp |
| **Bộ kết quả chuẩn** | **14.710 chuỗi phím** | Kiểm tra hồi quy: mỗi lần đổi luật gõ được so với bộ chuẩn này |

Đo trên Linux, bản release, cùng một máy, qua socket IBus (không gồm GNOME Shell và ứng dụng):

| | Vòng phím (trung vị) | Bộ nhớ | Binary |
|---|---|---|---|
| `rustype-linux` | 189–228 µs | khoảng 3 MB | khoảng 755 KB |
| `ibus-daemon` + engine cũ | 435–502 µs | khoảng 59 MB (cả tiến trình con) | — |

Con số vòng phím không gồm Mutter, GNOME Shell và ứng dụng, nên lợi ích thực tế khi gõ sẽ nhỏ hơn. Đã chạy thử trên một máy Ubuntu GNOME Wayland.

---

## 🌟 Điểm nổi bật

- ⚡ **Nhẹ và nhanh (Pure Rust):** lõi gõ không phụ thuộc thư viện ngoài. Trên Linux, `rustype-linux` là một file khoảng 755 KB, dùng khoảng 3 MB RAM.
- 💻 **Terminal không lặp chữ, không giật nháy:** giữ từ đang gõ ở dạng gạch chân và commit một lần khi hết từ. Không gửi phím Backspace giả, nên không làm hỏng shell history. Đã thử trên GNOME Wayland với Ptyxis, GNOME Terminal, kitty, Alacritty và WezTerm, và các công cụ dòng lệnh như Claude Code.
- 🎯 **Tự chọn cách hiện chữ theo ứng dụng:** ứng dụng báo có Surrounding Text (*Chrome, VS Code, Slack, LibreOffice, Firefox*) thì chữ hiện thẳng vào con trỏ, không gạch chân. Terminal và ứng dụng khác dùng gạch chân.
- 🖱️ **Bấm chuột hay đổi cửa sổ không mất chữ:** ký tự đang gõ dở được commit ngay khi bạn nhấp chuột ra ngoài hoặc chuyển cửa sổ.
- 🔄 **Nhớ trạng thái bật/tắt theo từng ứng dụng:** như EVKey, Rustype nhớ bạn đang bật tiếng Việt ở trình duyệt hoặc chat, nhưng giữ tiếng Anh khi chuyển sang terminal hoặc IDE.
- ⌨️ **Telex & VNI với nhiều tính năng:**
  - Đặt dấu kiểu mới (`hoà, khoẻ, thuý`) hoặc kiểu truyền thống (`hòa, khỏe, thúy`).
  - Tự trả lại từ tiếng Anh phổ biến (`window`, `software`, `docker`, `terminal`...) khi gõ xong.
  - Gõ đôi phím để thoát dấu (`ss`, `dd`), gõ tắt (Macro) và từ điển riêng (Custom Words).
- 🎛️ **Giao diện GTK4 Libadwaita & khay hệ thống:**
  - Bật/tắt tiếng Việt và đổi kiểu gõ từ menu trên thanh GNOME (Linux) hoặc khay hệ thống (Windows).
  - Ứng dụng Cài đặt viết bằng GTK4 + Libadwaita, tự thích ứng chế độ Sáng/Tối theo GNOME HIG.
- 🩺 **Chẩn đoán tự động:** `rustype-linux --doctor` kiểm tra phiên GNOME, drop-in, unit systemd, file địa chỉ, nguồn nhập, extension và cấu hình, rồi đưa ra gợi ý khắc phục.
- 🔒 **Quyền riêng tư:** engine không có mã mạng và không ghi nhật ký phím gõ. Trên Linux, ô mật khẩu và ô PIN luôn được bỏ qua. Ứng dụng Cài đặt chỉ có một yêu cầu mạng, khi bạn bấm "Kiểm tra cập nhật" (xem [PRIVACY.md](docs/PRIVACY.md)).

---

## 📦 Tải về & Cài đặt

Tải về phiên bản mới nhất tại **[GitHub Releases](https://github.com/tr-nam/rustype/releases)**.

### Trước khi cài trên Linux

Rustype thay ibus-daemon bằng một tiến trình riêng, `rustype-linux`. Vì vậy khi dùng Rustype, bạn **không** dùng được:

- Các bộ gõ IBus khác, ví dụ bộ gõ tiếng Nhật hoặc tiếng Trung. Chỉ có Rustype và bàn phím US chạy được.
- Emoji panel của IBus.
- Ứng dụng Flatpak dựa vào IBus portal. Chưa được kiểm chứng.
- Mọi desktop khác GNOME Shell, và GNOME trên X11.

Muốn quay về ibus-daemon và bộ gõ cũ, dùng lệnh gỡ cài đặt trong phần 1 bên dưới.

### 1. Linux (GNOME trên Wayland)

Chỉ hỗ trợ GNOME Shell trên Wayland. Các desktop và kiểu ứng dụng khác chưa được hỗ trợ, xem [COMPATIBILITY.md](docs/COMPATIBILITY.md).

**Gói `.deb` (Ubuntu / Debian):**
```bash
sudo apt install ./rustype_*.deb
```
Sau khi cài, Rustype hoạt động ngay trong phiên GNOME đang chạy, không cần đăng xuất. Nhấn Super+Space để chuyển bộ gõ.

**Cài từ mã nguồn** (cần Rust qua [rustup](https://rustup.rs)):
```bash
./install.sh
```
Script không cần sudo. Nó build `rustype-linux`, đưa tiến trình này vào vị trí của IBus trong phiên GNOME, tự kiểm tra, và tự khôi phục ibus nếu có lỗi.

Gói `ibus` vẫn phải được cài, vì GNOME Shell cần thư viện libibus. ibus-daemon không chạy khi dùng Rustype.

Để quay về ibus-daemon:
```bash
./uninstall.sh
```
Với gói `.deb`, gỡ bằng `sudo apt remove rustype`. Gói tự bỏ Rustype khỏi nguồn nhập và khởi động lại IBus, không cần đăng xuất.

### 2. Windows
1. Tải file `rustype.exe` hoặc `rustype-v*-windows-x86_64.zip` từ mục Releases.
2. Giải nén và chạy trực tiếp `rustype.exe` (không cần cài đặt).
3. Biểu tượng Rustype sẽ xuất hiện ở khay hệ thống (System Tray). Nhấp chuột để bật/tắt tiếng Việt (`V`/`E`) hoặc đổi kiểu gõ.

> [!NOTE]
> Bản Windows chưa được ký số. Nếu SmartScreen cảnh báo, chọn **More info**, rồi **Run anyway**.

### 3. macOS
1. Tải file `rustype-v*-macos.zip` từ mục Releases.
2. Giải nén được `Rustype.app`.
3. Chép `Rustype.app` vào thư mục bộ gõ người dùng:
   ```bash
   cp -R Rustype.app ~/Library/Input\ Methods/
   ```
4. Đăng xuất hoặc khởi động lại máy, sau đó vào **System Settings $\rightarrow$ Keyboard $\rightarrow$ Input Sources** và thêm **Rustype**.

> [!NOTE]
> Bản macOS chưa được ký và notarize. Nếu Gatekeeper chặn, nhấp chuột phải vào `Rustype.app` rồi chọn **Open**. Hoặc gỡ thuộc tính quarantine:
> ```bash
> xattr -dr com.apple.quarantine "$HOME/Library/Input Methods/Rustype.app"
> ```

---

## ⚙️ Sử dụng & Cấu hình

### Bật / Tắt gõ tiếng Việt
- **Linux**: nhấn **Super + Space** để chuyển nguồn nhập, hoặc nhấp vào biểu tượng bộ gõ trên thanh GNOME để gạt công tắc **"Bật tiếng Việt"**. Biểu tượng hiện `VN` hoặc `EN`.
- **Windows**: nhấn phím tắt chuyển đổi hoặc nhấp biểu tượng `V`/`E` ở khay hệ thống.
- **macOS**: chọn Rustype từ menu Input Sources ở thanh menu trên cùng.

### Tùy chỉnh cấu hình
File cấu hình tự động được đọc tại `~/.config/rustype/config` (Linux), hoặc chỉnh qua ứng dụng **Cài đặt Rustype** (`rustype-settings`):

```ini
mode = auto                 # auto (khuyên dùng) | preedit | direct
input_method = telex        # telex | vni
modern_orthography = false  # false: hòa, khỏe, thúy | true: hoà, khoẻ, thuý
auto_restore_english = true # tự trả lại từ tiếng Anh khi kết thúc từ
enable_macro = true         # bật tính năng gõ tắt
remember_state_per_app = true # nhớ trạng thái bật/tắt theo từng app (như EVKey)
custom_words = this, test   # danh sách từ tiếng Anh không thêm dấu
free_tone_order = true      # gõ dấu thanh f, j, x trước nguyên âm: thfi -> thì
toggle_shortcut = ctrl_shift # ctrl_shift | off: nhấn và thả Ctrl+Shift để bật/tắt tiếng Việt
simple_telex = false        # w đứng một mình không thành ư; aw, ow, uw vẫn hoạt động
quick_telex = false         # ở đầu từ: cc -> ch, gg -> gi, kk -> kh, nn -> ng, pp -> ph, qq -> qu, tt -> th
auto_capitalize = false     # tự viết hoa chữ đầu sau . ! ? và dấu cách
```

- `simple_telex`, `quick_telex`, `auto_capitalize`: mặc định đều tắt. Trên Linux, bạn cũng bật hoặc tắt các mục này trong ứng dụng Cài đặt.
- `auto_capitalize`: không áp dụng ở ô URL, ô mật khẩu và terminal. Chữ đầu tiên ngay sau khi vào một ô cũng không được viết hoa tự động.
- `[app_modes]`: ngoài `preedit` và `auto`, có thể đặt `off` để ứng dụng đó luôn tắt tiếng Việt, ví dụ `"org.gnome.Terminal" = off`. Tính năng này trên GNOME cần extension `rustype-focus` để nhận diện ứng dụng đang focus. Trên Linux, trang "Ứng dụng luôn tắt" trong ứng dụng Cài đặt cũng đặt được danh sách này.

- `free_tone_order`: trong Telex, các phím dấu thanh `f`, `j`, `x` gõ trước nguyên âm vẫn được áp dụng. Ví dụ `thfi` ra `thì`. Phím `s` và `r` không nằm trong danh sách này, vì chúng thường xuất hiện trong tiếng Anh.
- `toggle_shortcut`: trên GNOME, nhấn và thả Ctrl và Shift cùng lúc, không nhấn phím nào khác, để bật hoặc tắt tiếng Việt. Đặt `off` để tắt phím tắt này.

- `auto`: không gạch chân ở ứng dụng có Surrounding Text (trình duyệt, ứng dụng văn phòng), gạch chân trong terminal để tránh lặp chữ.
- `preedit`: luôn gạch chân ở mọi ứng dụng.
- `direct`: không gạch chân ở ứng dụng có Surrounding Text. Ứng dụng không có Surrounding Text vẫn dùng gạch chân như `preedit`, vì phím chuyển tiếp không hoạt động trong Chrome và VTE trên GNOME Wayland.

### Kiểm tra & Chẩn đoán hệ thống (Linux)
```bash
rustype-linux --doctor
```
Nếu Rustype không hoạt động, xem [TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md).

---

## 🏗️ Cấu trúc dự án (Workspace)

```
rustype/
├── rustype-core/       # Lõi Telex & VNI thuần (Pure Rust, không I/O, có C FFI)
│   ├── src/telex.rs    # Máy trạng thái ghép âm, bỏ dấu, khôi phục từ tiếng Anh
│   ├── src/tables.rs   # Bảng ngữ âm tiếng Việt chuẩn
│   ├── src/ffi.rs      # Giao tiếp C ABI cho các nền tảng khác nhúng vào
│   ├── include/        # rustype.h cho C/C++/Objective-C
│   ├── tests/          # Bộ kết quả chuẩn golden.tsv
│   └── benches/        # Benchmark hiệu năng (engine_perf.rs)
├── rustype-linux/      # Bus IBus và engine trong một tiến trình, cho GNOME trên Wayland
│   └── src/            # bus, server, sasl, wire, service, ime, config, dbus, props, ...
├── rustype-windows/    # Bộ gõ Windows (Low-Level Keyboard Hook & System Tray)
├── rustype-macos/      # Bộ gõ macOS InputMethodKit (Rustype.app bundle)
├── tools/              # Ứng dụng Cài đặt GTK4 + Libadwaita (rustype-settings) và script thử nghiệm
├── website/            # Trang giới thiệu và bộ gõ thử nghiệm trực tuyến
└── packaging/          # Đóng gói deb, rpm, PKGBUILD
```

---

## 📚 Tài liệu chi tiết

| Tài liệu | Đối tượng | Nội dung |
|---|---|---|
| [Trang chủ chính thức](https://rustype.namtr.dev) | Mọi người | Giới thiệu, tải gói cài đặt, hỏi đáp và gõ thử trực tuyến |
| [Tương thích](docs/COMPATIBILITY.md) | Mọi người | Phạm vi hỗ trợ, ứng dụng đã thử, cách hiện chữ |
| [Quyền riêng tư](docs/PRIVACY.md) | Mọi người | Engine không có mã mạng, không ghi nhật ký phím gõ, và quyền riêng tư của Surrounding Text |
| [Xử lý sự cố](docs/TROUBLESHOOTING.md) | Người dùng | Cách kiểm tra, gạch chân, lặp chữ, cách quay về ibus-daemon |
| [Kiến trúc](docs/ARCHITECTURE.md) | Nhà phát triển | Thiết kế một tiến trình, các module, đường đi của phím bấm |
| [Phát triển](docs/DEVELOPMENT.md) | Nhà phát triển | Build, kiểm thử, bộ kết quả chuẩn, gỡ lỗi và đóng gói |
| [Lộ trình phát triển](docs/ROADMAP.md) | Mọi người | Các hạng mục đã xong và đang làm |
| [Lịch sử thay đổi](CHANGELOG.md) | Mọi người | Chi tiết các bản cập nhật qua từng phiên bản |

---

## 📄 Giấy phép

Dự án được phân phối dưới giấy phép **MIT License**. Xem chi tiết tại [LICENSE](LICENSE).
Tác giả: [Nam Tran](https://namtr.dev) • Hỗ trợ: [support@namtr.dev](mailto:support@namtr.dev).

<p align="center">
  <img src="assets/banner.png" alt="Rustype - Gõ tiếng Việt cùng Rust" width="700">
</p>

<p align="center">
  <strong>Bộ gõ tiếng Việt đa nền tảng siêu nhẹ, hiệu năng cao viết hoàn toàn bằng Pure Rust cho Linux, Windows và macOS.</strong>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/language-Rust-orange.svg?style=flat-square&logo=rust" alt="Rust"></a>
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-blue.svg?style=flat-square" alt="Platform">
  <a href="https://github.com/tr-nam/rustype/actions/workflows/release.yml"><img src="https://img.shields.io/badge/build-passing-brightgreen.svg?style=flat-square" alt="Build Status"></a>
  <a href="https://github.com/tr-nam/rustype/releases"><img src="https://img.shields.io/badge/release-v0.1.0-blue.svg?style=flat-square" alt="Latest Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="License"></a>
</p>

---

Rustype là bộ gõ tiếng Việt thế hệ mới hỗ trợ kiểu gõ Telex & VNI, được xây dựng hoàn toàn bằng ngôn ngữ Rust thuần không phụ thuộc thư viện runtime bên ngoài.

Dự án cung cấp giải pháp gõ tiếng Việt đồng nhất, dứt khoát trên cả 3 nền tảng:
- **Linux (Ubuntu/Debian Wayland & X11)**: Tích hợp sâu qua IBus, giải quyết dứt điểm lỗi lặp chữ trong terminal (*Ptyxis, Alacritty, Ghostty, Claude Code*), không nháy màn hình, bấm chuột không mất chữ.
- **Windows**: Chạy độc lập qua cơ chế Low-Level Keyboard Hook (`SetWindowsHookExW` + `SendInput`) và khay hệ thống (System Tray), mượt mà trong mọi ứng dụng và game.
- **macOS**: Đóng gói chuẩn InputMethodKit (`Rustype.app`), tương thích hoàn toàn Apple Silicon (M-series) và Intel.

Trải nghiệm gõ thử trực tuyến tại trang chủ: **[namtr.dev](https://namtr.dev)**

---

## ⚡ Hiệu năng thực tế (Benchmarks)

Kiểm thử bằng bộ benchmark chuyên dụng (`cargo bench -p rustype-core`) trên profile `release`:

| Chỉ số kiểm thử | Kết quả thực tế | Ý nghĩa thực tế |
|---|---|---|
| **Throughput gõ từng phím** | **~2.520.000 keys/giây** | Gấp ~**170.000 lần** tốc độ gõ của con người (~15 keys/s) |
| **Throughput xử lý văn bản (feed)** | **~2.480.000 keys/s** (~2.4 MB/s) | Chuyển đổi hàng loạt tài liệu lớn tức thì |
| **Độ trễ trung vị (p50)** | **461 ns** (~0.46 $\mu$s) | Xử lý gần như bằng 0 trong một chu kỳ CPU |
| **Độ trễ phân vị cao (p99)** | **892 ns** (< 1 $\mu$s) | Cực kỳ ổn định, không bao giờ có độ trễ giật lag |
| **Độ trễ tối đa (Max latency)** | **20.5 $\mu$s** (0.02 ms) | Thấp hơn **50 lần** so với ngưỡng mắt người cảm nhận được (1 ms) |
| **Phục hồi từ tiếng Anh** | **~4.980.000 keys/giây** | Tự nhận diện và khôi phục từ tiếng Anh chỉ mất ~200 ns |
| **Kích thước Struct lõi** | **200 bytes** (`Telex`) | Bộ nhớ tiêu thụ cực thấp, RAM daemon ~**2.5 MB** |
| **Độ chính xác ngữ pháp** | **14.710 / 14.710 test cases** (100%) | Vượt qua toàn bộ bộ dữ liệu golden set chuẩn |

---

## 🌟 Điểm nổi bật

- ⚡ **Siêu nhẹ & Tốc độ tức thì (Pure Rust):** Lõi gõ được tối ưu hóa zero-allocation trên đường dẫn gõ phím nóng. Không phụ thuộc runtime GUI nặng nề, binary ~450 KB, tiêu thụ ~2.5 MB RAM.
- 💻 **Terminal không lặp chữ, không giật nháy:** Tối ưu hóa đặc biệt cho các terminal hiện đại trên Wayland (*Ptyxis, Alacritty, Ghostty, Foot, GNOME Terminal*) và các công cụ dòng lệnh (*Claude Code, Neovim, fzf*). Giữ từ đang gõ ở chế độ preedit mượt mà và commit dứt khoát khi kết thúc từ, hoàn toàn không gửi phím Backspace giả làm hỏng shell history hay giật màn hình.
- 🎯 **Tự động nhận diện ứng dụng (Auto Display Mode):** Tự động chuyển đổi giữa Direct mode và Preedit mode. Với các ứng dụng đồ họa có hỗ trợ Surrounding Text (*Chrome, VS Code, Slack, LibreOffice*), chữ được đưa thẳng vào vị trí con trỏ không có đường gạch chân.
- 🖱️ **Bấm chuột hay đổi cửa sổ không mất chữ:** Ký tự đang gõ dở được tự động hoàn tất (COMMIT) ngay khi bạn nhấp chuột ra ngoài hoặc chuyển cửa sổ làm việc.
- 🔄 **Nhớ trạng thái bật/tắt theo từng ứng dụng (Per-App State):** Tương tự EVKey, Rustype tự động ghi nhớ bạn đang bật gõ tiếng Việt ở trình duyệt/chat nhưng giữ nguyên chế độ tiếng Anh khi chuyển sang Terminal hoặc IDE lập trình.
- ⌨️ **Hỗ trợ đầy đủ Telex & VNI cùng tính năng thông minh:**
  - Tùy chọn đặt dấu kiểu mới (`hoà, khoẻ, thuý`) hoặc truyền thống (`hòa, khỏe, thúy`).
  - Tự động phục hồi từ tiếng Anh phổ biến (`window`, `software`, `docker`, `terminal`...) khi gõ xong.
  - Gõ đôi phím thoát dấu (`ss`, `dd`), gõ tắt (Macro) tốc độ cao và thêm từ điển riêng (Custom Words).
- 🎛️ **Giao diện GTK4 Libadwaita & System Tray:**
  - Bật/tắt tiếng Việt (icon động `VN`/`EN`) và đổi kiểu gõ trực tiếp từ khay hệ thống.
  - Ứng dụng Cài đặt hiện đại viết bằng GTK4 + Libadwaita, tự động thích ứng chế độ Sáng/Tối (Dark/Light style) chuẩn GNOME HIG.
- 🩺 **Chẩn đoán hệ thống tự động (Built-in Doctor):** Tích hợp sẵn công cụ kiểm tra toàn diện cấu hình IBus daemon, socket, component XML và extension (`ibus-engine-rustype --doctor` hoặc chạy trực tiếp từ trang Giới thiệu trong app Cài đặt).
- 🔒 **Bảo mật & Quyền riêng tư 100% (Offline & Zero Telemetry):** Hoàn toàn ngoại tuyến, không gửi telemetry, không lưu phím gõ và tự động bypass ở các trường nhập mật khẩu an toàn.

---

## 📦 Tải về & Cài đặt

Tải về phiên bản mới nhất tại **[GitHub Releases](https://github.com/tr-nam/rustype/releases)**:

### 1. Ubuntu / Debian (Linux)
Tải gói `.deb` từ Releases và cài đặt:
```bash
sudo apt install ./rustype_0.1.0-1_amd64.deb
```
Hoặc tự đóng gói từ mã nguồn:
```bash
make deb
sudo apt install ./target/packages/rustype_*.deb
```

### 2. Windows
1. Tải file `rustype.exe` hoặc `rustype-v*-windows-x86_64.zip` từ mục Releases.
2. Giải nén và chạy trực tiếp `rustype.exe` (không cần cài đặt).
3. Biểu tượng Rustype sẽ xuất hiện ở khay hệ thống (System Tray). Nhấp chuột để bật/tắt tiếng Việt (`V`/`E`) hoặc đổi kiểu gõ.

### 3. macOS
1. Tải file `rustype-v*-macos.zip` từ mục Releases.
2. Giải nén được `Rustype.app`.
3. Chép `Rustype.app` vào thư mục bộ gõ người dùng:
   ```bash
   cp -R Rustype.app ~/Library/Input\ Methods/
   ```
4. Đăng xuất hoặc khởi động lại máy, sau đó vào **System Settings $\rightarrow$ Keyboard $\rightarrow$ Input Sources** và thêm **Rustype**.

### 4. Cài đặt cho nhà phát triển (Linux source)
Yêu cầu Rust toolchain (`rustup`). Chạy script cài đặt tự động:
```bash
./install.sh
```

---

## ⚙️ Sử dụng & Cấu hình

### Bật / Tắt gõ tiếng Việt
- **Linux**: Nhấn phím tắt **Super + Space** hoặc click vào biểu tượng bộ gõ trên Top Bar để gạt công tắc **"Bật tiếng Việt"**. Icon chuyển đổi trực quan giữa `VN` và `EN`.
- **Windows**: Nhấn phím tắt chuyển đổi hoặc click biểu tượng `V`/`E` ở khay hệ thống.
- **macOS**: Chọn Rustype từ menu Input Sources ở thanh menu trên cùng.

### Tùy chỉnh cấu hình
File cấu hình tự động được đọc tại `~/.config/rustype/config` (Linux) hoặc quản lý trực tiếp qua ứng dụng đồ họa **Cài đặt Rustype** (`rustype-settings`):

```ini
mode = auto                 # auto (khuyên dùng) | preedit | direct
input_method = telex        # telex | vni
modern_orthography = false  # false: hòa, khỏe, thúy | true: hoà, khoẻ, thuý
auto_restore_english = true # tự trả lại từ tiếng Anh khi kết thúc từ
enable_macro = true         # bật tính năng gõ tắt
remember_state_per_app = true # nhớ trạng thái bật/tắt theo từng app (như EVKey)
custom_words = this, test   # danh sách từ tiếng Anh không thêm dấu
```

- `auto`: không gạch chân ở ứng dụng văn phòng/trình duyệt, hiển thị preedit gạch chân trong terminal để tránh lặp chữ.
- `preedit`: luôn gạch chân ở mọi ứng dụng.
- `direct`: luôn gửi ký tự trực tiếp.

### Kiểm tra & Chẩn đoán hệ thống (Linux)
Chạy lệnh chẩn đoán tự động:
```bash
ibus-engine-rustype --doctor
```
Gỡ cài đặt hoàn toàn:
```bash
./uninstall.sh
```

---

## 🏗️ Cấu trúc dự án (Workspace Architecture)

```
rustype/
├── rustype-core/       # Lõi Telex & VNI thuần (Pure Rust, 0 dependencies, C FFI headers)
│   ├── src/telex.rs    # Máy trạng thái ghép âm, bỏ dấu, khôi phục từ tiếng Anh
│   ├── src/tables.rs   # Bảng ngữ âm tiếng Việt chuẩn
│   ├── src/ffi.rs      # Giao tiếp C ABI cho các nền tảng khác nhúng vào
│   ├── include/        # rustype.h cho C/C++/Objective-C
│   └── benches/        # Benchmark hiệu năng thực tế (engine_perf.rs)
├── rustype-ibus/       # Daemon IBus cho Linux Wayland & X11
│   ├── src/ime.rs      # Xử lý phiên gõ, surrounding text, phím tắt
│   ├── src/ibus.rs     # Giao thức IBus engine
│   └── src/dbus.rs     # Client D-Bus siêu nhẹ thuần Rust
├── rustype-windows/    # Bộ gõ Windows (Low-Level Keyboard Hook & System Tray)
├── rustype-macos/      # Bộ gõ macOS InputMethodKit (Rustype.app bundle)
├── tools/              # Ứng dụng Cài đặt GTK4 + Libadwaita (rustype-settings)
├── website/            # Trang giới thiệu portal & bộ gõ thử nghiệm trực tiếp
└── packaging/          # Đóng gói deb, rpm, PKGBUILD
```

---

## 📚 Tài liệu chi tiết

| Tài liệu | Đối tượng | Nội dung |
|---|---|---|
| [Trang chủ chính thức](https://namtr.dev) | Mọi người | Giới thiệu, tải gói cài đặt, hỏi đáp và gõ thử nghiệm trực tuyến |
| [Tương thích](docs/COMPATIBILITY.md) | Mọi người | Ma trận ứng dụng, cơ chế hiển thị chữ, cấu hình theo từng ứng dụng |
| [Quyền riêng tư](docs/PRIVACY.md) | Mọi người | Cam kết 100% ngoại tuyến, zero telemetry, không ghi phím gõ |
| [Xử lý sự cố](docs/TROUBLESHOOTING.md) | Người dùng | Hướng dẫn chẩn đoán, gạch chân, lặp chữ, xử lý lỗi thường gặp |
| [Kiến trúc](docs/ARCHITECTURE.md) | Nhà phát triển | Thiết kế hệ thống, mô hình đa nền tảng, đường đi của phím bấm |
| [Phát triển](docs/DEVELOPMENT.md) | Nhà phát triển | Hướng dẫn build, chạy kiểm thử, tạo release và gỡ lỗi |
| [Lộ trình phát triển](docs/ROADMAP.md) | Mọi người | Kế hoạch hoàn thiện các giai đoạn từ bản 0.1 đến 1.0 |
| [Lịch sử thay đổi](CHANGELOG.md) | Mọi người | Chi tiết các bản cập nhật qua từng phiên bản |

---

## 📄 Giấy phép

Dự án được phân phối dưới giấy phép **MIT License**. Xem chi tiết tại [LICENSE](LICENSE).
Tác giả: [Nam Tran](https://namtr.dev) • Hỗ trợ: [support@namtr.dev](mailto:support@namtr.dev).

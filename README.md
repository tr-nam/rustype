<p align="center">
  <img src="assets/banner.png" alt="Rustype - Gõ tiếng Việt cùng Rust" width="700">
</p>

<p align="center">
  <strong>Bộ gõ tiếng Việt siêu nhẹ, hiệu năng cao viết bằng Rust cho Linux (IBus & Wayland).</strong>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/language-Rust-orange.svg?style=flat-square&logo=rust" alt="Rust"></a>
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20Wayland%20%7C%20X11-blue.svg?style=flat-square&logo=linux" alt="Platform">
  <img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="License">
</p>

Bộ gõ tiếng Việt Telex và VNI cho IBus, viết hoàn toàn bằng Rust, không phụ thuộc thư viện runtime bên ngoài.
Thiết kế tối ưu cho Ubuntu GNOME trên Wayland, gõ mượt mà không lỗi trong terminal (Ptyxis, Alacritty, Claude Code) và các ứng dụng đồ họa (VS Code, Chrome, LibreOffice).

## Điểm nổi bật

- ⚡ **Siêu nhẹ & Tốc độ tức thì (Pure Rust):** Tiến trình daemon được viết hoàn toàn bằng Rust thuần không phụ thuộc runtime GUI nặng nề, chiếm chưa đến **2.5 MB RAM**, kích thước tệp thực thi chỉ ~**450 KB** và xử lý mỗi phím gõ chỉ mất khoảng **0.3 micro giây** ($\mu$s).
- 💻 **Gõ trong Terminal không lặp chữ, không giật nháy:** Tối ưu hóa đặc biệt cho các terminal hiện đại trên Wayland (*Ptyxis, Alacritty, Ghostty, GNOME Terminal, Foot*) và các công cụ dòng lệnh (*Claude Code, Neovim, fzf*). Giữ từ đang gõ ở chế độ preedit mượt mà và đẩy ký tự dứt khoát khi kết thúc từ, hoàn toàn không gửi phím Backspace giả làm hỏng shell history hay giật màn hình.
- 🎯 **Tự động nhận diện ứng dụng (Auto Display Mode):** Tự động chuyển đổi giữa Direct mode và Preedit mode. Với các ứng dụng đồ họa có hỗ trợ Surrounding Text (*Chrome, VS Code, Slack, LibreOffice*), chữ được đưa thẳng vào vị trí con trỏ không có đường gạch chân khó chịu.
- 🖱️ **Bấm chuột hay đổi cửa sổ không mất chữ:** Ký tự đang gõ dở được tự động hoàn tất (COMMIT) ngay khi bạn nhấp chuột ra ngoài hoặc chuyển cửa sổ làm việc, không bao giờ bị nuốt chữ hay sót ký tự.
- 🔄 **Nhớ trạng thái bật/tắt theo từng ứng dụng (Per-App State):** Tương tự tính năng được yêu thích trên EVKey, Rustype tự động ghi nhớ bạn đang bật gõ tiếng Việt ở trình duyệt/chat nhưng giữ nguyên chế độ tiếng Anh khi chuyển sang Terminal hoặc IDE lập trình.
- ⌨️ **Hỗ trợ đầy đủ Telex & VNI cùng tính năng thông minh:**
  - Tùy chọn đặt dấu kiểu mới (`hoà, khoẻ, thuý`) hoặc truyền thống (`hòa, khỏe, thúy`).
  - Tự động phục hồi từ tiếng Anh phổ biến (`window`, `software`, `user`...) khi gõ xong.
  - Gõ đôi phím thoát dấu (`ss`, `dd`), gõ tắt (Macro) tốc độ cao và thêm từ điển riêng (Custom Words).
- 🎛️ **Thao tác nhanh trên Top Bar & Giao diện GTK4 Libadwaita:**
  - Bật/tắt tiếng Việt (chuyển đổi icon trạng thái `VN`/`EN` động) và đổi kiểu gõ Telex/VNI chỉ với 1 cú nhấp chuột ngay trên khay hệ thống GNOME.
  - Ứng dụng Cài đặt hiện đại viết bằng GTK4 + Libadwaita, tự động thích ứng chế độ Sáng/Tối (Dark/Light style) chuẩn GNOME HIG.
- 🩺 **Chẩn đoán hệ thống tự động (Built-in Doctor):** Tích hợp sẵn công cụ kiểm tra toàn diện cấu hình IBus daemon, socket, component XML và extension (`ibus-engine-rustype --doctor` hoặc chạy trực tiếp từ trang Giới thiệu trong app Cài đặt).
- 🔒 **Bảo mật & Quyền riêng tư 100% (Offline & Zero Telemetry):** Hoàn toàn ngoại tuyến, không gửi telemetry, không lưu phím gõ và tự động bypass ở các trường nhập mật khẩu an toàn.

## Cài đặt

### Cách 1: Cài đặt bằng gói `.deb` (Khuyên dùng cho Ubuntu / Debian)
Tải gói `.deb` từ mục [Releases](https://github.com/tr-nam/rustype/releases) hoặc tự đóng gói nhanh bằng:
```bash
make deb
sudo apt install ./target/packages/rustype_*.deb
```

### Cách 2: Cài đặt từ mã nguồn (Dành cho nhà phát triển)
Cần Rust (`rustup`). Chạy script cài đặt:
```bash
./install.sh
```

## Sử dụng & Cấu hình

### Bật / Tắt gõ tiếng Việt
- **Cách 1 (Phím tắt)**: Nhấn tổ hợp phím **Super + Space** (hoặc phím tắt chuyển nguồn nhập của bạn) để chuyển đổi qua lại giữa bàn phím tiếng Anh và Rustype.
- **Cách 2 (Menu thanh trạng thái GNOME)**: Bấm vào biểu tượng bộ gõ trên thanh Top Bar và gạt công tắc **"Bật tiếng Việt"**. Biểu tượng sẽ chuyển đổi linh hoạt giữa **"VN"** và **"EN"**.

### Đổi kiểu gõ (Telex / VNI) & Tùy biến
- **Cách 1 (Menu thanh trạng thái GNOME)**: Bấm vào biểu tượng bộ gõ trên thanh Top Bar và nhấp chọn trực tiếp **(•) Kiểu gõ Telex** hoặc **( ) Kiểu gõ VNI**.
- **Cách 2 (Giao diện đồ họa)**: Mở ứng dụng **Cài đặt Rustype** từ danh sách ứng dụng hoặc gõ lệnh `rustype-settings`.
- **Cách 3 (File cấu hình)**: Chỉnh sửa file `~/.config/rustype/config`, lưu xong bấm sang ô gõ khác là có hiệu lực ngay:

```ini
mode = auto                 # auto | preedit | direct
input_method = telex        # telex | vni
modern_orthography = false  # false: hòa, khỏe, thúy   true: hoà, khoẻ, thuý
auto_restore_english = true # tự trả lại từ tiếng Anh
enable_macro = true         # gõ tắt từ ~/.config/rustype/macro
remember_state_per_app = true # nhớ trạng thái bật/tắt theo từng app (như EVKey)
custom_words = this, test   # từ tiếng Anh không bị thêm dấu
```

- `auto`: không gạch chân ở ứng dụng thường, gạch chân trong terminal. Khuyên dùng.
- `preedit`: luôn gạch chân ở mọi ứng dụng.
- `direct`: không bao giờ gạch chân. Terminal trên Wayland sẽ bị lặp chữ.

Ô mật khẩu luôn được bỏ qua, phím đi thẳng vào ứng dụng.

### Kiểm tra & Chẩn đoán hệ thống
- Chạy lệnh: `ibus-engine-rustype --doctor` (hoặc mở Cài đặt Rustype và nhấn nút "Kiểm tra hệ thống").

Gỡ cài đặt bằng `./uninstall.sh` hoặc `sudo apt remove rustype`.

## Tài liệu & Liên kết

| Tài liệu | Dành cho | Nội dung |
|---|---|---|
| [Trang chủ chính thức](https://namtr.dev) | Mọi người | Giới thiệu, tải gói cài đặt, hỏi đáp và gõ thử nghiệm trực tiếp |
| [Tương thích](docs/COMPATIBILITY.md) | Mọi người | Ma trận ứng dụng, cơ chế hiện chữ, tùy biến cấu hình theo ứng dụng (app_modes) |
| [Quyền riêng tư](docs/PRIVACY.md) | Mọi người | Cam kết 100% ngoại tuyến, không telemetry, không keylogging |
| [Xử lý sự cố](docs/TROUBLESHOOTING.md) | Người dùng | Kiểm tra cài đặt, gạch chân, lặp chữ, từ tiếng Anh bị thêm dấu, báo lỗi |
| [Kiến trúc](docs/ARCHITECTURE.md) | Người phát triển | Các module, đường đi của một phím, ba cách hiện chữ, lõi Telex, giao thức IBus |
| [Phát triển](docs/DEVELOPMENT.md) | Người phát triển | Build, ba mức kiểm thử, sinh bảng luật, gỡ lỗi trên máy thật |
| [Lộ trình](docs/ROADMAP.md) | Mọi người | Kế hoạch từ bản 0.1 tới bản 1.0 cho Linux, rồi Windows và macOS |
| [Thay đổi](CHANGELOG.md) | Mọi người | Những gì thay đổi qua từng phiên bản |

## Giấy phép

Rustype dùng giấy phép MIT, xem [LICENSE](LICENSE).

# Thay đổi

Định dạng theo [Keep a Changelog](https://keepachangelog.com/vi/1.1.0/), phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Chưa phát hành]

### Thêm
- Tài liệu: kiến trúc, phát triển, xử lý sự cố, lộ trình, bảng tương thích ứng dụng (`docs/COMPATIBILITY.md`).
- File giấy phép MIT cho Rustype.
- Bộ kết quả chuẩn `rustype-core/tests/golden.tsv` gồm 14.710 chuỗi phím, kiểm tra trong `cargo test`, và công cụ `tools/golden.py` để xem và ghi nhận khác biệt khi đổi luật gõ.
- Cargo workspace: `rustype-core` (lõi Telex thuần, không phụ thuộc nền tảng) và `rustype-ibus` (engine IBus).
- CI GitHub Actions: kiểm tra định dạng, clippy, unit test, golden test, IBus giả lập.
- Fuzz target cho lõi Telex bằng `cargo-fuzz`.
- Xử lý `SetSurroundingText` và selection autocomplete cho thanh địa chỉ Chrome / Firefox / Electron.
- Hỗ trợ cấu hình chế độ hiển thị riêng cho từng ứng dụng qua mục `[app_modes]` trong `config`.
- Interface D-Bus `SetFocusApp` và GNOME Shell extension `rustype-focus@namtr.dev` để nhận diện app focus trên Wayland.
- C FFI cho `rustype-core` (`rustype.h`, thư viện tĩnh `librustype_core.a` và động `librustype_core.so`).
- Khung addon cho Fcitx5 (`rustype-fcitx5/`).
- Kiểu gõ VNI (`input_method = vni`) với luật gõ số và phím lặp thoát dấu.
- Bảng gõ tắt (Macro) tương thích định dạng file Unikey (`~/.config/rustype/macro` hoặc mục `[macros]`).
- Danh sách từ tiếng Anh người dùng tự thêm (`custom_words` hoặc file `~/.config/rustype/words`).
- Tính năng nhớ trạng thái bật/tắt tiếng Việt theo từng ứng dụng (`remember_state_per_app = true`, giống EVKey).

### Đổi
- Chế độ gỡ lỗi (`RUSTYPE_DEBUG`) chỉ in mã phím, không in chữ người dùng gõ.

### Bỏ
- Công cụ so sánh với engine bên ngoài và công cụ sinh bảng luật. `rustype-core/src/tables.rs` giờ được sửa trực tiếp.

## [0.1.0] - 2026-11-07

Bản đầu tiên, dùng trên Ubuntu 26.04, GNOME 50, Wayland.

### Thêm
- Engine IBus `ibus-engine-rustype` viết bằng Rust, không dùng thư viện ngoài, có lớp D-Bus tự viết.
- Kiểu gõ Telex với cách đặt dấu kiểu cũ hoặc kiểu mới, tự trả lại từ tiếng Anh, gõ đôi để thoát.
- Chọn cách hiện chữ theo ứng dụng: không gạch chân khi ứng dụng có Surrounding Text, gạch chân ở chế độ COMMIT trong terminal.
- Bỏ qua ô mật khẩu và ô mã PIN.
- File cấu hình `~/.config/rustype/config`, tự nạp lại khi sửa.
- Script `install.sh` và `uninstall.sh`.
- Gõ trong terminal trên GNOME Wayland không bị lặp chữ.
- Bấm chuột hoặc đổi cửa sổ lúc đang gõ dở không làm mất chữ.
- Kiểm thử: unit test và IBus giả lập.

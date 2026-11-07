# Thay đổi

Định dạng theo [Keep a Changelog](https://keepachangelog.com/vi/1.1.0/), phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Chưa phát hành]

### Thêm
- Tài liệu: kiến trúc, phát triển, xử lý sự cố, lộ trình.
- File giấy phép MIT cho Rustype.
- Bộ kết quả chuẩn `tests/golden.tsv` gồm 14.710 chuỗi phím, kiểm tra trong `cargo test`, và công cụ `tools/golden.py` để xem và ghi nhận khác biệt khi đổi luật gõ.
### Bỏ
- Công cụ so sánh với engine bên ngoài và công cụ sinh bảng luật. `src/tables.rs` giờ được sửa trực tiếp.

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

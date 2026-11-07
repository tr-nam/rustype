# Rustype

Bộ gõ tiếng Việt Telex cho IBus, viết bằng Rust, không dùng thư viện ngoài nào.
Làm riêng cho máy Ubuntu GNOME trên Wayland, gõ nhiều trong terminal (Ptyxis, Claude Code) và VS Code.

## Điểm nổi bật

- **Gõ trong terminal không lặp chữ.** Terminal không cho bộ gõ sửa chữ đã gõ, nên Rustype giữ từ đang gõ ở dạng gạch chân và gửi đi khi hết từ, không gửi phím xóa lùi giả.
- **Bấm chuột không mất chữ.** Chữ gạch chân được gửi ở chế độ COMMIT, nên GNOME giữ lại từ đang gõ dở khi bạn bấm chuột hoặc đổi cửa sổ.
- **Không gạch chân ở ứng dụng thường.** Ứng dụng có Surrounding Text như trình duyệt hay VS Code nhận chữ trực tiếp, chỉ terminal mới gạch chân.
- **Rất nhẹ.** Khoảng 2,5 MB RAM, file chạy khoảng 450 KB, mỗi phím xử lý trong khoảng 0,3 micro giây.
- **Telex đầy đủ.** Đặt dấu kiểu cũ hoặc kiểu mới, tự trả lại từ tiếng Anh như "window", "software", "user", gõ đôi phím để thoát như "ss" hay "dd".

## Cài đặt

Cần Rust (`rustup`). Chạy một lần, script sẽ hỏi mật khẩu sudo để chép file đăng ký vào `/usr/share/ibus/component`:

```
./install.sh
```

Sau đó nhấn Super+Space để chọn Rustype, biểu tượng là "VN".
Sửa code xong thì chạy lại `./install.sh` để cập nhật. Gỡ bằng `./uninstall.sh`.

Giao diện Cài đặt đồ họa: mở **Cài đặt Rustype** từ màn hình ứng dụng (hoặc chạy `rustype-settings`).
Chẩn đoán hệ thống tự động: chạy `target/release/ibus-engine-rustype --doctor`.

## Cấu hình

File `~/.config/rustype/config`, sửa xong bấm sang ô gõ khác là có hiệu lực:

```
mode = auto                 # auto | preedit | direct
input_method = telex        # telex | vni
modern_orthography = false  # false: hòa, khỏe, thúy   true: hoà, khoẻ, thuý
auto_restore_english = true # tự trả lại từ tiếng Anh
enable_macro = true         # gõ tắt từ ~/.config/rustype/macro
remember_state_per_app = true # nhớ trạng thái bật/tắt theo từng app (như EVKey)
custom_words = this, test   # từ tiếng Anh không bị thêm dấu
```

- `auto`: không gạch chân ở ứng dụng thường, gạch chân trong terminal. Nên dùng.
- `preedit`: luôn gạch chân.
- `direct`: không bao giờ gạch chân. Terminal trên Wayland sẽ bị lặp chữ.

Ô mật khẩu luôn được bỏ qua, phím đi thẳng vào ứng dụng.

## Tài liệu

| Tài liệu | Dành cho | Nội dung |
|---|---|---|
| [Tương thích](docs/COMPATIBILITY.md) | Mọi người | Ma trận ứng dụng, cơ chế hiện chữ, tùy biến cấu hình theo ứng dụng (app_modes) |
| [Xử lý sự cố](docs/TROUBLESHOOTING.md) | Người dùng | Kiểm tra cài đặt, gạch chân, lặp chữ, từ tiếng Anh bị thêm dấu, báo lỗi |
| [Kiến trúc](docs/ARCHITECTURE.md) | Người phát triển | Các module, đường đi của một phím, ba cách hiện chữ, lõi Telex, giao thức IBus |
| [Phát triển](docs/DEVELOPMENT.md) | Người phát triển | Build, ba mức kiểm thử, sinh bảng luật, gỡ lỗi trên máy thật |
| [Lộ trình](docs/ROADMAP.md) | Mọi người | Kế hoạch từ bản 0.1 tới bản 1.0 cho Linux, rồi Windows và macOS |
| [Thay đổi](CHANGELOG.md) | Mọi người | Những gì thay đổi qua từng phiên bản |

## Giấy phép

Rustype dùng giấy phép MIT, xem [LICENSE](LICENSE).

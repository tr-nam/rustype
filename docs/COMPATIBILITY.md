# Bảng tương thích ứng dụng (Compatibility Matrix)

Rustype hỗ trợ cơ chế hiện chữ thích ứng linh hoạt theo từng loại ứng dụng trên Linux (Wayland & X11).

## Ma trận ứng dụng đã kiểm nghiệm

| Nhóm | Ứng dụng | Chế độ mặc định (`auto`) | Ghi chú & Trạng thái |
|---|---|---|---|
| **Terminal** | Ptyxis (GNOME) | Preedit (gạch chân) | Hoạt động hoàn hảo. Chế độ COMMIT ngăn lặp chữ trên Wayland. |
| | GNOME Terminal | Preedit (gạch chân) | Ổn định. |
| | Konsole (KDE) | Preedit | Ổn định. |
| | kitty | Preedit | Hoạt động tốt. |
| | Alacritty | Preedit | Hoạt động tốt. |
| | WezTerm | Preedit | Hoạt động tốt. |
| **Trình duyệt** | Google Chrome / Chromium | Surrounding Text (không gạch chân) | Hoạt động mượt mà. Đã xử lý bôi đen autocomplete thanh địa chỉ (Omnibox). |
| | Mozilla Firefox | Surrounding Text (không gạch chân) | Ô tìm kiếm và thanh URL gõ đúng dấu, không nuốt chữ. |
| **Electron** | Visual Studio Code | Surrounding Text | Soạn thảo code và thanh Command Palette (Ctrl+P / Ctrl+Shift+P) chuẩn xác. |
| | Obsidian | Surrounding Text | Gõ Markdown mượt mà, không giật con trỏ. |
| | Slack / Discord | Surrounding Text | Soạn thảo tin nhắn tức thì không gạch chân. |
| **Khác** | LibreOffice Writer | Surrounding Text | Nhận chữ trực tiếp, dấu đặt đúng vị trí. |
| | Telegram Desktop | Surrounding Text | Ổn định. |
| | JetBrains IDEs | Surrounding Text | Hoạt động tốt khi bật IBus support. |
| | Ứng dụng Flatpak | Tùy quyền hạn IBus của Flatpak | Thường tự động nhận IBus qua portal. |

---

## Các chế độ hiện chữ (`mode`)

1. **`auto`** (Mặc định):
   - Ứng dụng hỗ trợ Surrounding Text (Chrome, VS Code, LibreOffice...): nhận chữ trực tiếp, không gạch chân.
   - Ứng dụng không có Surrounding Text (Terminal Ptyxis, kitty...): hiển thị gạch chân (Preedit ở chế độ COMMIT), nhấn Space/Enter để hoàn tất từ. Không bao giờ gửi phím Backspace giả, không bị lặp chữ.
2. **`preedit`**: Luôn gạch chân trong mọi ứng dụng. An toàn và ổn định nhất nếu gặp ứng dụng lỗi surrounding text.
3. **`direct`**: Không bao giờ gạch chân. Terminal trên Wayland có thể bị lặp chữ.

---

## Tùy biến theo ứng dụng (`app_modes`)

Nếu có một ứng dụng xử lý Surrounding Text không chuẩn (ví dụ một game hay một app Java cũ), bạn có thể ép ứng dụng đó dùng `preedit` trong file `~/.config/rustype/config`:

```ini
mode = auto
modern_orthography = false
auto_restore_english = true

[app_modes]
org.gnome.Ptyxis = preedit
kitty = preedit
"google-chrome" = auto
```

### Nhận diện ứng dụng trên GNOME Wayland

Trên GNOME Wayland, tất cả ứng dụng kết nối tới IBus thông qua GNOME Shell compositor. Để IBus engine nhận biết chính xác ứng dụng nào đang focus:
- Cài đặt extension `gnome-shell-extension/rustype-focus@namtr.dev` (script `./install.sh` tự động cài đặt).
- Extension sẽ thông báo `app_id` qua D-Bus tới Rustype mỗi khi bạn chuyển cửa sổ.

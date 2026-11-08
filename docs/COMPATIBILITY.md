# Bảng tương thích ứng dụng (Compatibility Matrix)

## Phạm vi hỗ trợ

**Linux: chỉ GNOME Shell trên Wayland.** Rustype chạy như bus IBus trong `rustype-linux` và được kiểm tra trên GNOME. Windows và macOS có bản riêng, xem [README](../README.md).

Đã chạy thử trên một máy Ubuntu GNOME Wayland thật. Chưa thử trên máy hay phiên bản GNOME khác.

### Không được hỗ trợ

Các mục dưới đây không được hỗ trợ và chưa có kế hoạch hỗ trợ:

- **Ứng dụng Flatpak.** Chưa có ibus-portal, nên ứng dụng Flatpak không nhận bộ gõ của Rustype.
- **KDE Plasma, Sway, Hyprland** và các desktop khác (XFCE, Cinnamon, MATE...). Không kiểm thử và không hứa hoạt động.
- **Engine IBus khác.** `Factory.CreateEngine` chưa được hỗ trợ. Chỉ có `rustype` và `xkb:us::eng` chạy được.
- **Emoji panel của IBus.**
- **XIM** (ứng dụng X11 thuần).
- **Fcitx5.** Không còn addon Fcitx5.

Gói `ibus` vẫn được cài vì GNOME Shell cần libibus. ibus-daemon không chạy khi dùng Rustype.

## Ma trận ứng dụng đã kiểm nghiệm

Bảng này ghi kết quả thử với bộ gõ cũ (ibus-daemon cùng engine). `rustype-linux` dùng cùng lõi gõ và cùng `ime.rs`, nhưng chưa được thử lại từng ứng dụng trong bảng này.

| Nhóm | Ứng dụng | Chế độ mặc định (`auto`) | Ghi chú |
|---|---|---|---|
| **Terminal** | Ptyxis | Preedit (gạch chân) | Chế độ COMMIT ngăn lặp chữ trên Wayland. |
| | GNOME Terminal | Preedit (gạch chân) | Ổn định. |
| | kitty | Preedit | Hoạt động tốt. |
| | Alacritty | Preedit | Hoạt động tốt. |
| | WezTerm | Preedit | Hoạt động tốt. |
| **Trình duyệt** | Google Chrome / Chromium | Surrounding Text (không gạch chân), kể cả thanh địa chỉ (URL) | Ô thường và thanh địa chỉ đều hoạt động. Ở thanh địa chỉ, khi Chrome bỏ qua lệnh xóa vì đang chọn gợi ý, engine xóa phần đuôi và commit lại từ. Chữ sai có thể thoáng hiện trong chốc lát. |
| | Mozilla Firefox | Surrounding Text (không gạch chân) | Ô tìm kiếm và thanh URL gõ đúng dấu, không nuốt chữ. |
| **Electron** | Visual Studio Code | Surrounding Text | Soạn thảo code và thanh Command Palette (Ctrl+P / Ctrl+Shift+P). |
| | Obsidian | Surrounding Text | Gõ Markdown, không giật con trỏ. |
| | Slack / Discord | Surrounding Text | Soạn thảo tin nhắn không gạch chân. |
| **Khác** | LibreOffice Writer | Surrounding Text | Nhận chữ trực tiếp, dấu đặt đúng vị trí. |
| | Telegram Desktop | Surrounding Text | Ổn định. |
| | JetBrains IDEs | Surrounding Text | Hoạt động khi bật IBus support. |

---

## Các chế độ hiện chữ (`mode`)

1. **`auto`** (mặc định):
   - Ứng dụng có Surrounding Text (Chrome, VS Code, LibreOffice...): chữ hiện trực tiếp, không gạch chân. Ô có mục đích URL (thanh địa chỉ) cũng dùng Surrounding Text, kèm cơ chế riêng để sửa khi Chrome bỏ qua lệnh xóa lúc có gợi ý tự động. Cơ chế này chỉ bật cho ô URL.
   - Ứng dụng không có Surrounding Text (Ptyxis, kitty...): hiển thị gạch chân (preedit ở chế độ COMMIT), gõ Space hoặc Enter để hoàn tất từ. Không gửi phím Backspace giả, nên không lặp chữ.
2. **`preedit`**: luôn gạch chân trong mọi ứng dụng. An toàn và ổn định nhất nếu gặp ứng dụng có Surrounding Text lỗi.
3. **`direct`**: không gạch chân ở ứng dụng có Surrounding Text. Ứng dụng không có Surrounding Text dùng preedit, như `auto`. Rustype không còn gửi phím Backspace giả trong chế độ này, vì phím chuyển tiếp không hoạt động trong Chrome và VTE trên GNOME Wayland.

---

## Tùy biến theo ứng dụng (`app_modes`)

Nếu một ứng dụng xử lý Surrounding Text không chuẩn (ví dụ một game hay một app Java cũ), bạn có thể ép ứng dụng đó dùng `preedit` trong file `~/.config/rustype/config`:

```ini
mode = auto
modern_orthography = false
auto_restore_english = true

[app_modes]
org.gnome.Ptyxis = preedit
kitty = preedit
"google-chrome" = auto
```

### Nhận diện ứng dụng đang focus

Trên GNOME Wayland, các ứng dụng kết nối tới IBus qua GNOME Shell, nên engine không tự biết ứng dụng nào đang focus. Extension `gnome-shell-extension/rustype-focus@namtr.dev` gửi tên ứng dụng cho Rustype qua `SetFocusApp` mỗi khi bạn đổi cửa sổ. Nếu không có extension, mục `[app_modes]` không áp dụng được theo từng ứng dụng.

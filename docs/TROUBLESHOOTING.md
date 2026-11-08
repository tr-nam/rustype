# Xử lý sự cố

Rustype trên Linux chỉ hỗ trợ GNOME Shell trên Wayland. Nếu máy bạn dùng desktop khác, xem [COMPATIBILITY.md](COMPATIBILITY.md) trước.

## Kiểm tra nhanh

Chạy công cụ chẩn đoán:

```bash
rustype-linux --doctor
```

Công cụ kiểm tra cài đặt, bus, file đăng ký, nguồn nhập GNOME, extension và file cấu hình, rồi đưa ra gợi ý khắc phục cho từng mục bị lỗi.

Nếu muốn kiểm tra thủ công:

| Lệnh | Kết quả đúng |
|---|---|
| `systemctl --user status org.freedesktop.IBus.session.GNOME.service` | Trạng thái `active (running)`, và tiến trình chính là `rustype-linux` |
| `pgrep -ax rustype-linux` | Một dòng có đường dẫn tới `rustype-linux` |
| `gsettings get org.gnome.desktop.input-sources sources` | Có `('ibus', 'rustype')` trong danh sách |
| `echo $XDG_SESSION_TYPE` | `wayland` |

## Xem nhật ký

Nhật ký của tiến trình nằm trong journal của systemd user:

```bash
journalctl --user -u org.freedesktop.IBus.session.GNOME.service -n 200
```

Thêm `-f` để xem nhật ký theo thời gian thực khi thử lại. Nhật ký không chứa nội dung bạn gõ, trừ khi bạn bật chế độ gỡ lỗi (xem mục dưới).

## Không thấy Rustype khi nhấn Super+Space

- Nếu `rustype-linux --doctor` báo file đăng ký hoặc nguồn nhập bị thiếu: chạy lại `./install.sh` trong thư mục dự án (bản cài từ mã nguồn), hoặc cài lại gói `.deb`.
- Nếu có Rustype trong danh sách nhưng không chọn được: vào Cài đặt, mục Bàn phím, thêm nguồn nhập "Rustype (Tiếng Việt)".
- Vừa cài gói `.deb` mà vẫn không thấy: chạy `rustype-linux --activate --restart` trong terminal, rồi thử lại. Lần đăng nhập GNOME tiếp theo cũng tự kích hoạt Rustype.

## Đã chọn Rustype nhưng gõ không ra dấu

1. Kiểm tra biểu tượng trên thanh trên cùng có phải "VN" không.
2. Chạy `pgrep -ax rustype-linux`. Nếu không có gì, tiến trình không chạy. Xem nhật ký bằng lệnh `journalctl` ở trên để biết lý do.
3. Nếu vẫn không được, nhấn Super+Space để chuyển sang nguồn nhập khác rồi chuyển lại.
4. Ô mật khẩu luôn gõ không dấu. Đây là cố ý.

## Chữ bị gạch chân khi đang gõ

Trong terminal như Ptyxis, GNOME Terminal hay Claude Code, chữ gạch chân là bình thường. Terminal không cho bộ gõ sửa chữ đã gõ, nên Rustype giữ từ đang gõ ở dạng gạch chân và gửi đi khi bạn gõ hết từ.

Ở ứng dụng thường như trình duyệt hay VS Code, chữ không gạch chân. Nếu muốn khác đi, sửa `mode` trong `~/.config/rustype/config`:

| Giá trị | Kết quả |
|---|---|
| `auto` | Gạch chân trong terminal, không gạch chân ở nơi khác. Mặc định |
| `preedit` | Luôn gạch chân. Ổn định nhất ở mọi ứng dụng |
| `direct` | Không gạch chân khi ứng dụng có Surrounding Text. Ứng dụng khác vẫn gạch chân, vì phím chuyển tiếp không hoạt động trên GNOME Wayland |

Sửa xong, bấm sang ô gõ khác là có hiệu lực.

## Một ứng dụng bị lặp chữ hoặc mất chữ

1. Đặt `mode = preedit` rồi thử lại trong ứng dụng đó.
2. Nếu hết lỗi, hãy báo lại tên ứng dụng để Rustype xử lý riêng cho nó, ví dụ bằng `[app_modes]`.
3. Nếu vẫn lỗi, xem nhật ký và báo cáo theo mục bên dưới.

## Từ tiếng Anh bị thêm dấu

Rustype tự trả lại nhiều từ tiếng Anh như "window", "user", "software". Một số từ tiếng Anh lại cũng là âm tiết tiếng Việt hợp lệ, ví dụ "this" thành "thí", nên không trả lại được.

- Gõ phím dấu hai lần để bỏ dấu: gõ "thiss" sẽ ra "this".
- Gõ đoạn dài tiếng Anh thì nhấn Super+Space để chuyển sang bàn phím tiếng Anh.
- Muốn tắt hẳn việc tự trả lại từ tiếng Anh: đặt `auto_restore_english = false`.

## Muốn đổi kiểu gõ giữa Telex và VNI

- **Cách 1**: Mở ứng dụng **Cài đặt Rustype** (hoặc chạy `rustype-settings`), ở thẻ Chung chọn kiểu gõ mong muốn.
- **Cách 2**: Chỉnh dòng `input_method = vni` (hoặc `telex`) trong file `~/.config/rustype/config`.

## Muốn đặt dấu kiểu mới như hoà, thuý

Đặt `modern_orthography = true` trong `~/.config/rustype/config`, hoặc bật công tắc "Dấu kiểu mới" trong ứng dụng Cài đặt.

## Bấm chuột khi đang gõ dở thì mất chữ

Rustype được làm để không xảy ra lỗi này. Nếu bạn gặp, hãy ghi lại tên ứng dụng, phiên bản GNOME và kết quả `rustype-linux --doctor`, rồi báo lại theo mục dưới.

## Quay về ibus-daemon

Nếu muốn dùng lại bộ gõ IBus mặc định:

```bash
./uninstall.sh
```

Chạy lệnh này trong thư mục dự án khi cài từ mã nguồn. Script xóa drop-in của Rustype và khởi động lại IBus. Cấu hình trong `~/.config/rustype` được giữ lại.

Nếu đã cài gói `.deb`, gỡ gói bằng `sudo apt remove rustype`. Gói tự bỏ Rustype khỏi nguồn nhập và khởi động lại IBus, không cần đăng xuất.

Nếu `./uninstall.sh` không chạy được, làm thủ công:

1. Xóa file `~/.config/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/override.conf`.
2. Chạy `systemctl --user daemon-reload`.
3. Khởi động lại `org.freedesktop.IBus.session.GNOME.service`, hoặc đăng xuất và đăng nhập lại.

Lưu ý: khi `rustype-linux` lỗi, không có bộ gõ nào hoạt động cho tới khi gỡ drop-in. Nên giữ sẵn một terminal để chạy các lệnh trên.

## Thu log để báo lỗi

Chế độ gỡ lỗi (`RUSTYPE_DEBUG=1`) in mã phím ra nhật ký của tiến trình. Nó tuân thủ quyền riêng tư: chỉ ghi mã phím dạng hex và trạng thái, **không in nội dung văn bản bạn gõ hay mật khẩu**.

Xem [DEVELOPMENT.md](DEVELOPMENT.md#gỡ-lỗi-trên-máy-thật) để biết cách bật chế độ này.

> [!NOTE]
> Nhật ký vẫn chứa mã phím, và có thể suy ra nội dung bạn gõ. Đọc lại trước khi gửi cho người khác.

Khi báo lỗi, ghi kèm:

- Tên và phiên bản Rustype.
- Desktop và phiên bản GNOME, cùng kết quả `echo $XDG_SESSION_TYPE`.
- Chuỗi phím đã gõ, kết quả mong đợi và kết quả thực tế.
- Kết quả của `rustype-linux --doctor`.

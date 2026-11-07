# Xử lý sự cố

## Kiểm tra nhanh

Chạy bốn lệnh sau. Nếu kết quả khớp cột bên phải thì Rustype đã cài và chạy đúng.

| Lệnh | Kết quả đúng |
|---|---|
| `ibus list-engine \| grep rustype` | `rustype - Rustype (Tiếng Việt)` |
| `gsettings get org.gnome.desktop.input-sources sources` | Có `('ibus', 'rustype')` trong danh sách |
| `ibus engine` | `rustype` khi đang chọn Rustype |
| `pgrep -ax ibus-engine-rus` | Một dòng có `ibus-engine-rustype --ibus` |

## Không thấy Rustype khi nhấn Super+Space

- Nếu `ibus list-engine` không có rustype: file đăng ký chưa được cài. Chạy lại `./install.sh` trong thư mục dự án.
- Nếu có rustype nhưng không có trong danh sách của GNOME: vào Cài đặt, mục Bàn phím, thêm nguồn nhập "Rustype (Tiếng Việt)". Chạy lại `./install.sh` cũng làm việc này.
- Vừa cài xong mà vẫn không thấy: chạy `ibus restart`, đợi vài giây rồi thử lại.

## Đã chọn Rustype nhưng gõ không ra dấu

1. Kiểm tra biểu tượng trên thanh trên cùng có phải "VN" không.
2. Chạy `pgrep -ax ibus-engine-rus`. Nếu không có gì, engine chưa chạy: chạy `ibus restart`.
3. Nếu vẫn không được, nhấn Super+Space để chuyển sang bộ gõ khác rồi chuyển lại.
4. Ô mật khẩu luôn gõ không dấu. Đây là cố ý.

## Chữ bị gạch chân khi đang gõ

Trong terminal như Ptyxis, GNOME Terminal hay Claude Code, chữ gạch chân là bình thường. Terminal không cho bộ gõ sửa chữ đã gõ, nên Rustype giữ từ đang gõ ở dạng gạch chân và gửi đi khi bạn gõ hết từ.

Ở ứng dụng thường như trình duyệt hay VS Code, chữ không gạch chân. Nếu bạn muốn khác đi, sửa `mode` trong `~/.config/rustype/config`:

| Giá trị | Kết quả |
|---|---|
| `auto` | Gạch chân trong terminal, không gạch chân ở nơi khác. Mặc định |
| `preedit` | Luôn gạch chân. Ổn định nhất ở mọi ứng dụng |
| `direct` | Không bao giờ gạch chân. Terminal trên Wayland sẽ bị lặp chữ |

Sửa xong, bấm sang ô gõ khác là có hiệu lực.

## Một ứng dụng bị lặp chữ hoặc mất chữ

Một số ứng dụng báo là sửa được chữ nhưng thực ra không làm đúng. Cách xử lý:

1. Đặt `mode = preedit` rồi thử lại trong ứng dụng đó.
2. Nếu hết lỗi, hãy báo lại tên ứng dụng để Rustype xử lý riêng cho nó trong bản sau.
3. Nếu vẫn lỗi, xem mục thu log bên dưới.

## Từ tiếng Anh bị thêm dấu

Rustype tự trả lại nhiều từ tiếng Anh như "window", "user", "software". Một số từ tiếng Anh lại cũng là âm tiết tiếng Việt hợp lệ, ví dụ "this" thành "thí", nên không trả lại được.

- Gõ phím dấu hai lần để bỏ dấu: gõ "thiss" sẽ ra "this".
- Gõ đoạn dài tiếng Anh thì nhấn Super+Space để chuyển sang bàn phím tiếng Anh.
- Muốn tắt hẳn việc tự trả lại từ tiếng Anh: đặt `auto_restore_english = false`.

## Muốn đặt dấu kiểu mới như hoà, thuý

Đặt `modern_orthography = true` trong `~/.config/rustype/config`.

## Bấm chuột khi đang gõ dở thì mất chữ

Rustype được làm để không xảy ra lỗi này. Nếu bạn gặp, hãy ghi lại tên ứng dụng, đang dùng Wayland hay X11, và gửi kèm log theo mục dưới.

## Thu log để báo lỗi

Xem cách chạy engine ở chế độ gỡ lỗi trong [DEVELOPMENT.md](DEVELOPMENT.md#gỡ-lỗi-trên-máy-thật).

> Log có thể chứa mật khẩu và mọi chữ bạn gõ. Hãy tái hiện lỗi bằng chữ thử, rồi đọc lại log trước khi gửi.

Khi báo lỗi, ghi kèm:

- Tên và phiên bản ứng dụng.
- Desktop, ví dụ GNOME hay KDE, và đang dùng Wayland hay X11. Lệnh `echo $XDG_SESSION_TYPE` cho biết điều này.
- Chuỗi phím đã gõ, kết quả mong đợi và kết quả thực tế.
- Kết quả của `ibus-engine-rustype --version`, chạy từ `~/.local/lib/rustype/`.

## Gỡ cài đặt

Chạy `./uninstall.sh` trong thư mục dự án. Script bỏ Rustype khỏi danh sách bộ gõ, xóa file đăng ký bằng sudo, khởi động lại IBus và xóa engine. Cấu hình trong `~/.config/rustype` được giữ lại.

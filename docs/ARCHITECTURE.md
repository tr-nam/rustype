# Kiến trúc

Rustype là một tiến trình duy nhất, `ibus-engine-rustype`, chạy một luồng và không dùng thư viện ngoài.
ibus-daemon khởi chạy nó theo file component `/usr/share/ibus/component/rustype.xml` khi người dùng chọn bộ gõ.

```
Ứng dụng
   │  Wayland text-input (GNOME Shell), GTK/Qt IM module, hoặc XIM
   ▼
ibus-daemon
   │  bus D-Bus riêng của IBus (socket trong ~/.cache/ibus)
   ▼
ibus-engine-rustype
   ├─ dbus.rs    đọc/ghi message D-Bus
   ├─ ibus.rs    object Factory và Engine, xử lý từng message theo thứ tự đến
   ├─ ime.rs     chọn cách hiện chữ cho từng ứng dụng, xử lý phím đặc biệt
   ├─ telex.rs   ghép chữ Telex, đặt dấu, kiểm tra chính tả
   ├─ tables.rs  bảng luật chính tả
   └─ config.rs  đọc ~/.config/rustype/config
```

## Các module

| File | Trách nhiệm | Phụ thuộc |
|---|---|---|
| `src/telex.rs` | Lõi Telex thuần, không I/O. Nhận từng ký tự, trả về "xóa N ký tự rồi chèn chuỗi S" | `tables.rs` |
| `src/tables.rs` | Bảng âm đầu, vần, âm cuối, bảng dấu, danh sách từ tiếng Anh | không |
| `src/ime.rs` | Một phiên gõ: chọn cách hiện chữ, xử lý BackSpace, Enter, phím tắt, ô mật khẩu | `telex.rs`, `config.rs` |
| `src/dbus.rs` | Client D-Bus tối giản: xác thực, đóng gói, đọc message | thư viện chuẩn |
| `src/ibus.rs` | Giao thức IBus: Factory, Engine, Properties, đóng gói IBusText | `dbus.rs`, `ime.rs` |
| `src/config.rs` | Đọc file cấu hình khi thời điểm sửa của file thay đổi | `telex.rs` |
| `src/main.rs` | Dòng lệnh, tìm địa chỉ IBus, in file component | tất cả |

Lõi `telex.rs` không biết gì về IBus. Đây là phần sẽ được tách thành crate riêng để dùng lại cho Fcitx5, Windows và macOS, xem [ROADMAP.md](ROADMAP.md).

## Đường đi của một phím

1. ibus-daemon gọi `ProcessKeyEvent(keyval, keycode, state)` trên object engine.
2. `ibus.rs` đọc ba số và gọi `Ime::process_key`.
3. `ime.rs` phân loại phím:
   - Phím nhả và phím bổ trợ như Shift: bỏ qua, để ứng dụng xử lý.
   - Có giữ Ctrl, Alt hoặc Super, hoặc đang ở ô mật khẩu: commit chữ đang gõ dở, xóa trạng thái, để ứng dụng xử lý phím.
   - BackSpace: sửa chữ đang gõ, xem bên dưới.
   - Ký tự in được: đưa vào lõi Telex. Chữ cái thì ghép, ký tự khác thì kết thúc từ.
   - Phím còn lại như Enter, Tab, mũi tên: commit chữ đang gõ dở rồi để ứng dụng xử lý.
4. Kết quả là một danh sách lệnh `Out`: `Commit`, `Preedit`, `HidePreedit`, `DeleteSurrounding`, `ForwardKey`.
5. `ibus.rs` đóng gói từng lệnh thành tín hiệu D-Bus, thêm câu trả lời `true` hoặc `false` vào cuối, rồi ghi tất cả bằng một lệnh `write`.

Tín hiệu luôn được gửi trước câu trả lời. Vì vậy ibus-daemon và GNOME Shell xử lý chữ được commit trước khi trả phím Enter hay Space về cho ứng dụng, và chữ không bao giờ bị đảo thứ tự.

Lõi Telex xử lý mỗi phím trong khoảng 0,3 micro giây. Phần lớn độ trễ thực tế nằm ở ibus-daemon và GNOME Shell, không ở engine.

## Ba cách đưa chữ lên màn hình

Cách hiện chữ được chọn khi bắt đầu một từ và giữ nguyên tới hết từ đó.

| Cách | Khi nào dùng | Tín hiệu gửi đi | Người dùng thấy |
|---|---|---|---|
| Surrounding | Ứng dụng báo có Surrounding Text, tức hầu hết ứng dụng GTK, Qt, trình duyệt | `DeleteSurroundingText` rồi `CommitText` | Không gạch chân |
| Preedit | Ứng dụng không có Surrounding Text, ví dụ terminal dùng VTE như Ptyxis | `UpdatePreeditText` ở chế độ COMMIT, rồi `CommitText` khi hết từ | Chữ gạch chân tới khi hết từ |
| Backspace | Chỉ khi cấu hình `mode = direct` và ứng dụng không có Surrounding Text | `ForwardKeyEvent` phím BackSpace rồi `CommitText` | Không gạch chân, có thể lặp chữ |

Với `mode = auto`, mặc định, chỉ hai cách đầu được dùng.

**Vì sao không dùng phím BackSpace giả.** Trên GNOME Wayland, phím BackSpace giả không tới được terminal VTE. Bộ gõ nào dùng cách này sẽ gõ "kiểm" ra "Kieêmểm".

**Vì sao preedit phải ở chế độ COMMIT.** Khi người dùng bấm chuột hoặc đổi cửa sổ, Mutter gọi `clutter_input_focus_reset`. Hàm này commit chữ gạch chân nếu nó ở chế độ COMMIT, và vứt đi nếu ở chế độ CLEAR. GNOME Shell bật `client_commit_preedit`, nên ibus-daemon không commit thêm lần nữa.

Hệ quả là engine **không bao giờ tự commit** khi nhận `Reset` hay `FocusOut`. Nó chỉ xóa trạng thái. Nếu engine cũng commit thì từ đó sẽ bị gõ hai lần. Với ứng dụng X11, ibus-daemon tự commit đúng một lần, đã có kiểm thử cho trường hợp này.

**Ẩn preedit.** `HidePreedit` được gửi dưới dạng một `UpdatePreeditText` rỗng ở chế độ CLEAR, chứ không phải `HidePreeditText`. Cách này xóa luôn bản sao chữ gạch chân mà ibus-daemon và GNOME Shell còn giữ, nên một lần reset về sau không thể commit lại từ cũ.

## Lõi Telex

Trạng thái của một từ:

- `raw`: các phím đã gõ, dùng để trả lại từ tiếng Anh.
- `chars`: chữ cái gốc, nguyên âm đã có mũ, râu hoặc trăng nhưng chưa có dấu thanh.
- `tone`: dấu thanh từ 0 đến 5, đặt lên nguyên âm phù hợp mỗi khi hiển thị.
- `disabled`: từ đã bị coi là không phải tiếng Việt và giữ nguyên như đang thấy.
- `prev`: từ vừa kết thúc, để BackSpace qua dấu cách thì sửa lại được từ đó. Chỉ dùng ở cách Surrounding.

Quy tắc chính:

- **Đặt dấu thanh:** "ươ" thì dấu lên "ơ". Có nguyên âm mang mũ, râu hoặc trăng như â, ơ, ă thì dấu lên nguyên âm đó. Vần hai nguyên âm có âm cuối thì dấu lên nguyên âm sau. Vần "oa", "oe", "uy" không có âm cuối thì dấu lên nguyên âm đầu theo kiểu cũ, hoặc nguyên âm sau theo kiểu mới. Vần ba nguyên âm thì dấu lên nguyên âm giữa.
- **Gõ đôi để thoát:** "ss", "aaa", "ww", "ddd" trả về phím gốc, ví dụ "pass" và "dd".
- **Kiểm tra chính tả khi đang gõ:** khi chuỗi không còn có thể là tiếng Việt, ví dụ có "f", "j", "w", "z", âm cuối sai, hoặc có hai cụm nguyên âm, từ được trả về đúng các phím đã gõ.
- **Kiểm tra khi hết từ:** nếu từ đã bị biến đổi mà không phải âm tiết tiếng Việt hoàn chỉnh, hoặc nằm trong danh sách từ tiếng Anh, thì trả lại các phím gốc. Không làm vậy nếu người dùng đã BackSpace trong từ đó.

Hành vi mong đợi được ghi trong `tests/golden.tsv`, gồm 14.710 chuỗi phím và chữ phải ra trên màn hình, và `cargo test` kiểm tra toàn bộ.

## Lớp D-Bus

- **Tìm địa chỉ:** biến `IBUS_ADDRESS`, rồi file trong `IBUS_ADDRESS_FILE`, rồi `~/.config/ibus/bus/<machine-id>-<host>-<display>`, giống cách libibus làm. Trên Wayland, phần display là giá trị `WAYLAND_DISPLAY`.
- **Kết nối:** socket Unix, xác thực SASL EXTERNAL bằng uid lấy từ `/proc/self`, gọi `Hello`, rồi `RequestName("org.freedesktop.IBus.Rustype")` với cờ không xếp hàng.
- **Message:** gửi đi theo little-endian, đọc được cả hai kiểu. Không hỗ trợ truyền file descriptor vì IBus không cần.
- **Object:** `/org/freedesktop/IBus/Factory` có `CreateEngine` và `Destroy`. Mỗi engine nằm ở `/org/freedesktop/IBus/Engine/<n>`.
- **Thuộc tính:** `FocusId` và `ActiveSurroundingText` đều trả về `false`. `ContentType` được đọc để nhận biết ô mật khẩu.
- **IBusText:** gửi dưới dạng variant `(sa{sv}sv)`, bên trong là `IBusAttrList` kiểu `(sa{sv}av)`. Chữ gạch chân có thêm một `IBusAttribute` gạch chân đơn.
- **Lỗi:** nếu xử lý phím bị panic, engine dựng lại trạng thái, giữ thông tin ứng dụng, và để ứng dụng tự xử lý phím đó. Gõ chữ không bao giờ bị chặn hẳn.
- **Thoát:** khi ibus-daemon đóng kết nối, ví dụ lúc `ibus restart`, engine thoát êm. ibus-daemon sẽ chạy lại nó khi cần.

## Cấu hình

`config.rs` chỉ đọc lại file khi thời điểm sửa của file thay đổi, và chỉ kiểm tra lúc tạo engine hoặc lúc ứng dụng nhận focus. Cấu hình mới áp dụng từ từ tiếp theo, không cắt ngang từ đang gõ.

## Bảo mật và quyền riêng tư

- Engine không mở kết nối mạng và không ghi file nào khi chạy.
- Ô có mục đích PASSWORD hoặc PIN được bỏ qua hoàn toàn: mọi phím đi thẳng vào ứng dụng.
- Biến `RUSTYPE_DEBUG` in mã từng phím và chữ được commit ra stderr. Log này có thể chứa mật khẩu và nội dung riêng tư, chỉ bật khi đang gỡ lỗi. Kế hoạch bỏ phần in chữ nằm trong [ROADMAP.md](ROADMAP.md).

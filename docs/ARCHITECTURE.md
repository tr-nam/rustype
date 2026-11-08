# Kiến trúc (Architecture)

Rustype là một Cargo workspace. Trên Linux, toàn bộ đường đi của một phím nằm trong **một tiến trình duy nhất**, `rustype-linux`. Tiến trình này vừa là bus IBus, vừa chứa engine gõ. Nó thay cho `ibus-daemon` cộng `ibus-engine-rustype` cũ, nên mỗi phím bớt một tiến trình và một chặng IPC.

Phạm vi trên Linux chỉ là GNOME Shell trên Wayland. Xem [COMPATIBILITY.md](COMPATIBILITY.md).

## Các crate

- **`rustype-core`**: lõi Telex & VNI, không I/O, không phụ thuộc nền tảng. Có C FFI (`include/rustype.h`) để Windows và macOS dùng lại.
- **`rustype-linux`**: bus IBus tự chứa cùng engine, một tiến trình, chỉ dùng thư viện chuẩn của Rust. Đây là đường cài đặt mặc định trên Linux.
- **`rustype-windows`**: bộ gõ Windows, dùng Low-Level Keyboard Hook (`SetWindowsHookExW` + `SendInput`) và khay hệ thống (System Tray).
- **`rustype-macos`**: lớp vỏ InputMethodKit (`IMKServer` và `IMKInputController`), liên kết tĩnh với `librustype_core.a`.
- **`tools/`**: script thử nghiệm, công cụ kiểm tra bộ kết quả chuẩn và ứng dụng cài đặt `rustype-settings`.

## Sơ đồ

```
Ứng dụng (GTK, Qt, trình duyệt, terminal)
   │  Wayland text-input qua GNOME Shell, hoặc IM module GTK/Qt
   ▼
GNOME Shell (libibus client)
   │  D-Bus, qua socket trong file địa chỉ của IBus
   ▼
rustype-linux (một tiến trình, một luồng poll)
   ├─ server.rs, wire.rs, sasl.rs   nhận và gửi khung message
   ├─ bus.rs                         định tuyến theo tên và match rule
   ├─ service.rs                     object IBus, input context, panel
   ├─ ime.rs, props.rs               engine: từng phím, menu thuộc tính
   ├─ config.rs                      đọc ~/.config/rustype/config
   └─ rustype-core: telex.rs, tables.rs
```

Socket mặc định là `$XDG_RUNTIME_DIR/rustype/bus`, còn đường dẫn được ghi vào file địa chỉ kiểu ibus-daemon để client libibus tìm thấy.

## Các module

| File | Trách nhiệm | Phụ thuộc |
|---|---|---|
| `rustype-core/src/telex.rs` | Lõi Telex thuần, không I/O. Nhận từng ký tự, trả về "xóa N ký tự rồi chèn chuỗi S" | `tables.rs` |
| `rustype-core/src/tables.rs` | Bảng âm đầu, vần, âm cuối, bảng dấu, danh sách từ tiếng Anh | không |
| `rustype-linux/src/main.rs` | Dòng lệnh (`--address`, `--replace`, `--own-session-name`, `--version`), ghi file địa chỉ, dọn dẹp khi thoát | tất cả |
| `rustype-linux/src/server.rs` | Vòng lặp `poll` một luồng, đọc và ghi không chặn, lùi áp lực đầu vào | `wire.rs`, `sys.rs` |
| `rustype-linux/src/sasl.rs` | Bắt tay SASL EXTERNAL phía server | `sys.rs` |
| `rustype-linux/src/wire.rs` | Tách khung message theo header, mã hóa lại có sender và destination | `dbus.rs` |
| `rustype-linux/src/bus.rs` | Tên unique và well-known, match rule, định tuyến, RequestName, ReleaseName, NameHasOwner, ListNames, AddMatch, GetId | `wire.rs` |
| `rustype-linux/src/service.rs` | Object `org.freedesktop.IBus`, input context, client panel, `org.freedesktop.IBus.Rustype` (SetFocusApp) | `bus.rs`, `ime.rs`, `props.rs` |
| `rustype-linux/src/addrfile.rs` | File địa chỉ kiểu ibus-daemon. Từ chối khi file trỏ tới tiến trình đang sống, trừ khi có `--replace` | `sys.rs` |
| `rustype-linux/src/sys.rs` | `poll`, `SO_PEERCRED`, `kill`, bắt SIGINT và SIGTERM. Chỉ các hàm libc tối thiểu | libc |
| `rustype-linux/src/ime.rs` | Một phiên gõ: chọn cách hiện chữ (Surrounding hoặc Preedit), BackSpace, Enter, phím tắt, ô mật khẩu, ô URL | `rustype-core`, `config.rs` |
| `rustype-linux/src/config.rs` | Đọc `~/.config/rustype/config`, nạp lại khi thời điểm sửa của file thay đổi | `rustype-core` |
| `rustype-linux/src/dbus.rs` | Client D-Bus tối giản: đóng gói, đọc message, dùng cho `--own-session-name` | thư viện chuẩn |
| `rustype-linux/src/props.rs` | Thuộc tính menu trên thanh GNOME: bật tắt tiếng Việt, đổi Telex/VNI, mở Cài đặt (RegisterProperties, UpdateProperty) | `service.rs` |

Lõi `telex.rs` không biết gì về IBus. Đây là crate riêng để Windows và macOS dùng lại qua C FFI.

## Đường đi của một phím

1. GNOME Shell (libibus) gọi `ProcessKeyEvent(keyval, keycode, state)` trên input context.
2. `service.rs` đọc ba số và gọi `Ime::process_key` trong `ime.rs`.
3. `ime.rs` phân loại phím:
   - Phím nhả và phím bổ trợ như Shift: bỏ qua, để ứng dụng xử lý.
   - Có giữ Ctrl, Alt hoặc Super, hoặc đang ở ô mật khẩu: commit chữ đang gõ dở, xóa trạng thái, để ứng dụng xử lý phím.
   - BackSpace: sửa chữ đang gõ, xem bên dưới.
   - Ký tự in được: đưa vào lõi Telex. Chữ cái thì ghép, ký tự khác thì kết thúc từ.
   - Phím còn lại như Enter, Tab, mũi tên: commit chữ đang gõ dở rồi để ứng dụng xử lý.
4. Kết quả là một danh sách lệnh `Out`: `Commit`, `Preedit`, `HidePreedit`, `DeleteSurrounding`, `RequireSurrounding`.
5. `service.rs` đóng gói từng lệnh thành tín hiệu D-Bus, thêm câu trả lời `true` hoặc `false` vào cuối, rồi ghi tất cả bằng một lệnh `write`.

Tín hiệu luôn được gửi trước câu trả lời. Nhờ vậy GNOME Shell xử lý chữ được commit trước khi trả phím Enter hay Space về cho ứng dụng, và chữ không bao giờ bị đảo thứ tự.

Lõi Telex xử lý mỗi phím dưới 1 micro giây (trung vị khoảng 0,46 µs, đo bằng `cargo bench`). Phần lớn độ trễ thực tế nằm ở GNOME Shell và ứng dụng, không nằm ở engine.

## Hai cách đưa chữ lên màn hình

Cách hiện chữ được chọn khi bắt đầu một từ và giữ nguyên tới hết từ đó.

| Cách | Khi nào dùng | Tín hiệu gửi đi | Người dùng thấy |
|---|---|---|---|
| Surrounding | Ứng dụng báo có Surrounding Text: hầu hết ứng dụng GTK, Qt, trình duyệt, kể cả ô có mục đích URL | `DeleteSurroundingText` rồi `CommitText`. Ở ô URL, nếu báo cáo cho thấy lệnh xóa bị bỏ qua thì xóa phần đuôi đã nhớ rồi commit lại từ | Không gạch chân |
| Preedit | Ứng dụng không có Surrounding Text, ví dụ terminal dùng VTE như Ptyxis, hoặc `mode = preedit` | `UpdatePreeditText` ở chế độ COMMIT, rồi `CommitText` khi hết từ | Chữ gạch chân tới khi hết từ |

Với `mode = auto`, mặc định, ứng dụng có Surrounding Text dùng cách đầu, còn lại dùng preedit. Với `mode = direct`, ứng dụng có Surrounding Text cũng dùng cách đầu, còn lại dùng preedit.

Rustype **không còn** cách thứ ba dựa trên phím BackSpace giả. Trên GNOME Wayland, phím chuyển tiếp không đến được Chrome (ô thanh địa chỉ) hay VTE, nên cách đó làm hỏng chữ, ví dụ "kiểm" thành "Kieêmểm" trong terminal. Trước đây `mode = direct` dùng cách đó; nay nó dùng preedit.

**Vì sao ô URL dùng Surrounding Text.** Preedit làm mất gợi ý đang chọn khi nhấn Enter, và phím chuyển tiếp không hoạt động trong Chrome trên GNOME Wayland. Vì vậy ô có mục đích URL (`purpose` = 5) vẫn dùng Surrounding Text và không gạch chân.

Chrome bỏ qua `DeleteSurroundingText` khi đang chọn gợi ý nội tuyến, nhưng vẫn áp dụng lệnh commit, và commit đó thay thế gợi ý. Chỉ ở ô URL, engine mới yêu cầu Surrounding Text (`RequireSurroundingText`), ghi nhớ phần đuôi mà ô có thể đang chứa, và khi báo cáo cho thấy lệnh xóa đã bị bỏ qua thì xóa phần đuôi đó rồi commit lại từ. Báo cáo của Chrome có thể chậm hơn lúc gõ, và engine chấp nhận điều đó. Chữ sai có thể thoáng hiện trong chốc lát. Các ô khác không dùng cơ chế này.

**Vì sao preedit phải ở chế độ COMMIT.** Khi người dùng bấm chuột hoặc đổi cửa sổ, Mutter gọi `clutter_input_focus_reset`. Hàm này commit chữ gạch chân nếu nó ở chế độ COMMIT, và vứt đi nếu ở chế độ CLEAR. GNOME Shell bật `client_commit_preedit`, nên phía IBus không commit thêm lần nữa.

Hệ quả là engine **không bao giờ tự commit** khi nhận `Reset` hay `FocusOut`. Nó chỉ xóa trạng thái. Nếu engine cũng commit thì từ đó sẽ bị gõ hai lần.

**Ẩn preedit.** `HidePreedit` được gửi dưới dạng một `UpdatePreeditText` rỗng ở chế độ CLEAR, chứ không phải `HidePreeditText`. Cách này xóa luôn bản sao chữ gạch chân mà GNOME Shell còn giữ, nên một lần reset về sau không thể commit lại từ cũ.

## Lõi Telex

Trạng thái của một từ:

- `raw`: các phím đã gõ, dùng để trả lại từ tiếng Anh.
- `chars`: chữ cái gốc, nguyên âm đã có mũ, râu hoặc trăng nhưng chưa có dấu thanh.
- `tone`: dấu thanh từ 0 đến 5, đặt lên nguyên âm phù hợp mỗi khi hiển thị.
- `disabled`: từ đã bị coi là không phải tiếng Việt và giữ nguyên như đang thấy.
- `prev`: từ vừa kết thúc. Backspace có thể mở lại từ này để sửa tiếp, khi con trỏ vẫn ở ngay sau nó. Chỉ dùng ở cách Surrounding.

Quy tắc chính:

- **Đặt dấu thanh:** "ươ" thì dấu lên "ơ". Có nguyên âm mang mũ, râu hoặc trăng như â, ơ, ă thì dấu lên nguyên âm đó. Vần hai nguyên âm có âm cuối thì dấu lên nguyên âm sau. Vần "oa", "oe", "uy" không có âm cuối thì dấu lên nguyên âm đầu theo kiểu cũ, hoặc nguyên âm sau theo kiểu mới. Vần ba nguyên âm thì dấu lên nguyên âm giữa.
- **Dấu móc trên "uu":** gõ `uuw` ra "ưu", và "Luuw" ra "Lưu". Phím móc đặt lên u đầu tiên.
- **Gõ đôi để thoát:** "ss", "aaa", "ww", "ddd" trả về phím gốc, ví dụ "pass" và "dd".
- **Kiểm tra chính tả khi đang gõ:** khi chuỗi không còn có thể là tiếng Việt, ví dụ có "f", "j", "w", "z", âm cuối sai, hoặc có hai cụm nguyên âm, từ được trả về đúng các phím đã gõ.
- **Kiểm tra khi hết từ:** nếu từ đã bị biến đổi mà không phải âm tiết tiếng Việt hoàn chỉnh, hoặc nằm trong danh sách từ tiếng Anh, thì trả lại các phím gốc. Không làm vậy nếu người dùng đã BackSpace trong từ đó.

Hành vi mong đợi được ghi trong `rustype-core/tests/golden.tsv`. Đây là bộ kết quả chuẩn dùng để phát hiện hồi quy: mỗi dòng là một chuỗi phím và chữ phải hiện ra. `cargo test` kiểm tra toàn bộ. Bộ này cho biết một lần sửa luật có đổi kết quả nào không, không phải là số đo độ chính xác tiếng Việt.

## Lớp D-Bus và IBus

- **Tìm địa chỉ:** biến `IBUS_ADDRESS`, rồi file trong `IBUS_ADDRESS_FILE`, rồi file `~/.config/ibus/bus/<machine-id>-unix-<display>`, giống cách libibus làm. Trên Wayland, phần display là giá trị `WAYLAND_DISPLAY`.
- **Socket:** chỉ hỗ trợ `unix:path=`. Abstract socket chưa được hỗ trợ.
- **Kết nối:** xác thực SASL EXTERNAL bằng uid lấy qua `SO_PEERCRED`, bắt đầu bằng `Hello`. Với `--own-session-name`, tiến trình còn giữ tên `org.freedesktop.IBus` trên session bus, là tên mà unit systemd cần.
- **Message:** đọc và ghi theo little-endian. Message big-endian được chuyển tiếp nguyên bytes, không thêm sender. Không hỗ trợ truyền file descriptor vì IBus không cần.
- **Object:** `/org/freedesktop/IBus` cho object IBus, `/org/freedesktop/IBus/Factory` có `CreateEngine` và `Destroy`. Mỗi input context có một object riêng dưới `/org/freedesktop/IBus/InputContext_<n>`.
- **Thuộc tính:** `FocusId` và `ActiveSurroundingText` đều trả về `false`. `ContentType` được đọc để nhận biết ô mật khẩu và ô URL.
- **Surrounding Text:** engine chỉ phát `RequireSurroundingText` trong ô có mục đích URL, để libibus gửi `SetSurroundingText`. Cờ yêu cầu này gắn với cả input context, mà GNOME Shell dùng chung một context cho mọi ứng dụng, nên sau lần đầu vào ô URL client cũng gửi báo cáo của các ô khác cho tới khi engine được Enable lại. Engine bỏ ngay các báo cáo đó. Với ô URL, engine so phần đuôi văn bản với phần đã nhớ trong bộ nhớ tạm để phát hiện lệnh xóa bị bỏ qua. Không lưu và không ghi log nội dung này.
- **Engine:** `rustype` và `xkb:us::eng` (xkb là passthrough).
- **IBusText:** gửi dưới dạng variant `(sa{sv}sv)`, bên trong là `IBusAttrList` kiểu `(sa{sv}av)`. Chữ gạch chân có thêm một `IBusAttribute` gạch chân đơn.
- **Panel:** khi có client panel (`org.freedesktop.IBus.Panel`), rustype-linux gọi FocusIn, FocusOut, RegisterProperties, UpdateProperty, ContentType và DestroyContext. Nó nhận PropertyActivate để bật tắt tiếng Việt, đổi Telex/VNI và mở Cài đặt.
- **Ứng dụng đang focus:** extension `gnome-shell-extension/rustype-focus@namtr.dev` gọi `org.freedesktop.IBus.Rustype.SetFocusApp` mỗi khi đổi cửa sổ. Nhờ đó `[app_modes]` áp dụng đúng theo ứng dụng.
- **Lỗi:** nếu xử lý phím bị panic (`catch_unwind`), engine dựng lại trạng thái, giữ thông tin ứng dụng, và để ứng dụng tự xử lý phím đó. Gõ chữ không bao giờ bị chặn hẳn.
- **Thoát:** `SIGINT` và `SIGTERM` được bắt. Khi thoát, socket và file địa chỉ được xóa. Nếu bus cũ đang sống, `--replace` ghi đè file địa chỉ nhưng không dừng tiến trình cũ.

### Chưa hỗ trợ trong bus này

- `Factory.CreateEngine`: engine IBus khác không chạy được. Chỉ có `rustype` và `xkb:us::eng`.
- ibus-portal: ứng dụng Flatpak chưa có bộ gõ.
- Emoji panel và XIM.
- `EffectivePostProcessKeyEvent`: nhận rồi bỏ qua. `ProcessKeyEvent` trả lời trực tiếp, không xếp hàng hậu xử lý.
- Kết nối session bus (`--own-session-name`) chỉ được giữ để giữ tên `org.freedesktop.IBus`, không phục vụ message nào khác.

## Cấu hình

`config.rs` chỉ đọc lại file khi thời điểm sửa của file thay đổi, và chỉ kiểm tra lúc tạo input context hoặc lúc ứng dụng nhận focus. Cấu hình mới áp dụng từ từ tiếp theo, không cắt ngang từ đang gõ.

## Tiếp quản dịch vụ IBus trên GNOME

GNOME khởi động IBus qua unit systemd user `org.freedesktop.IBus.session.GNOME.service`. Rustype đưa `rustype-linux --own-session-name` vào chỗ đó bằng một drop-in:

- **Cài từ mã nguồn** (`./install.sh`, không cần sudo): drop-in nằm trong `~/.config/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/override.conf`. Dòng `ExecStart=` trống xóa lệnh cũ, dòng sau trỏ tới `rustype-linux`. Script tự kiểm tra và tự khôi phục ibus nếu lỗi.
- **Gói cài đặt**: drop-in của hệ thống nằm tại `/usr/lib/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/rustype.conf`. Cần đăng xuất rồi đăng nhập lại.
- **Quay về ibus-daemon**: `./uninstall.sh` với bản cài từ mã nguồn. Gói cài đặt thì gỡ gói, rồi đăng xuất và đăng nhập lại.

`ibus` vẫn phải được cài, vì GNOME Shell cần libibus. ibus-daemon không chạy khi đang dùng Rustype.

## Bảo mật và quyền riêng tư

- Engine không có mã mạng và không mở kết nối nào ra ngoài. Socket chỉ là Unix socket cục bộ.
- Engine không ghi nội dung gõ ra file. Khi chạy, nó chỉ ghi file địa chỉ và socket của bus.
- Ô có mục đích PASSWORD hoặc PIN được bỏ qua hoàn toàn: mọi phím đi thẳng vào ứng dụng.
- Biến `RUSTYPE_DEBUG` in mã từng phím ra stderr. Log không chứa nội dung người dùng gõ, chỉ chứa mã phím (keyval) và số lượng ký tự.

## Số đo

Đo bản release trên cùng một máy Ubuntu GNOME Wayland, 5 lần mỗi bên, n=243 mỗi lần, tính vòng phím qua socket:

| | Trung vị | p95 |
|---|---|---|
| rustype-linux | 189–228 µs | 245–388 µs |
| ibus-daemon + ibus-engine-rustype (bản cũ) | 435–502 µs | 575–622 µs |

Bộ nhớ: khoảng 3 MB cho tiến trình rustype-linux, so với khoảng 59 MB cho ibus-daemon và các tiến trình con trên cùng một máy. Binary khoảng 755 KB.

Con số vòng phím **không bao gồm** Mutter, JavaScript của GNOME Shell và ứng dụng. Lợi ích đầu-cuối thực tế nhỏ hơn chênh lệch trên, và chưa được đo riêng.

Mới chạy trên một máy Ubuntu GNOME Wayland thật. Chưa thử rộng trên máy hay phiên bản GNOME khác.

## Kiểm thử

```
cargo test
tools/rustype_linux_sandbox.sh target/release/rustype-linux
```

`cargo test` gồm unit test, bộ kết quả chuẩn `golden.tsv`, và `tests/omnibox_model.rs`, chạy bộ kết quả chuẩn qua một mô hình của ô thanh địa chỉ Chrome.

Script sandbox chạy trong `dbus-run-session` riêng, với `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR` và `IBUS_ADDRESS_FILE` tạm. Nó không đụng vào `~/.config/ibus` hay session bus thật. Script chạy client libibus thật (`tools/ibus_client.py`), kiểm tra panel (`tools/rustype_linux_panel_test.py`), rồi kiểm tra file địa chỉ và socket đã được xóa khi nhận SIGTERM.

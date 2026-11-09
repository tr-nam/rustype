# Thay đổi

Định dạng theo [Keep a Changelog](https://keepachangelog.com/vi/1.1.0/), phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Chưa phát hành]

## [0.2.1] - 2026-11-09

### Sửa lỗi
- Gõ vào terminal dùng VTE (GNOME Console, GNOME Terminal, Ptyxis) với cấu hình mặc định có thể làm sập GNOME Shell trên GNOME 45 đến 48. Terminal giờ luôn dùng gạch chân, và Rustype chỉ xóa rồi gõ lại ở ô đã báo nội dung. Đã thử trên GNOME 46 và 50.
- Ứng dụng tự xóa nội dung ô lúc đang gõ dở không còn làm sập GNOME Shell. Nếu phím ngay sau đó làm thay chữ của từ cũ thì vẫn có thể ra chữ thừa, vì GTK chỉ báo thay đổi sau phím đó.
- Gõ đôi phím thanh rồi gõ tiếp phụ âm cuối không còn bị trả về phím thô: `tesst` ra `test`, `resst` ra `rest`.
- `rustype-linux --doctor` trả mã thoát 1 khi có mục lỗi.
- Ứng dụng Cài đặt: danh sách từ gõ tắt và từ riêng cập nhật ngay khi thêm hoặc xóa.
- Ứng dụng Cài đặt đọc file cấu hình giống bộ gõ, kể cả giá trị lạ và dòng `app_mode.<ứng dụng>`. File không phải UTF-8 không còn làm ứng dụng sập; khi lưu, file cũ được giữ lại thành `config.invalid`.
- Sửa một từ gõ tắt đang nằm trong file `macro` giờ có tác dụng. Từ gõ tắt kết thúc bằng dấu `"` không còn mất dấu.
- `uninstall.sh` dọn hết dù `systemctl` lỗi. `install.sh` không còn báo thất bại khi chỉ không đọc được nguồn nhập. Gói `.deb` chỉ báo sẵn sàng khi kích hoạt thành công.
- Windows: chữ hoa ghép không còn ra chữ thường (`Đ`, `Â`, `TÔI`), với cả Shift và Caps Lock. Chưa thử trên máy Windows thật.
- `install.sh` dùng được trên máy đã cài gói `.deb`.

### Thêm
- Bộ kiểm thử tích hợp cho `install.sh`, `uninstall.sh`, gói `.deb` và ứng dụng Cài đặt (`make test-integration`).
- Bộ kiểm thử đầu cuối trên GNOME Shell thật (`make test-e2e`, và `make test-e2e-46` cho GNOME 46 trong Docker).

## [0.2.0] - 2026-11-09

### Sửa lỗi
- Ứng dụng Cài đặt không còn xóa các tùy chọn nó không quản lý khi lưu cấu hình.
- Windows không còn bỏ qua phím do phần mềm khác chèn vào. Chỉ đầu ra của chính Rustype bị bỏ qua (đã biên dịch, chưa chạy thử trên Windows).
- `rustype-linux` tự chạy lại khi lỗi (`Restart=on-failure`).
- Sáu từ tiếng Anh giữ nguyên, không thêm dấu: is, of, or, was, her, here.
- Ô URL: gõ Backspace rất nhanh ngay sau một dấu có thể xóa nhầm chữ đúng. Đã sửa.

### Thêm
- Simple Telex (`simple_telex`) và Quick Telex (`quick_telex`), tắt theo mặc định.
- Tự viết hoa chữ đầu câu (`auto_capitalize`), tắt theo mặc định.
- Ứng dụng luôn tắt tiếng Việt: giá trị `off` trong `[app_modes]`, và trang "Ứng dụng luôn tắt" trong ứng dụng Cài đặt.
- Ứng dụng Cài đặt có công tắc cho các tùy chọn mới.
- Phím tắt bật tắt tiếng Việt Ctrl+Shift ngay trong bộ gõ trên GNOME (`toggle_shortcut = ctrl_shift`, đặt `off` để tắt).
- Gõ tự do: dấu thanh `f`, `j`, `x` gõ trước nguyên âm vẫn có hiệu lực, như `thfi` ra `thì` (`free_tone_order = true`).

## [0.1.1] - 2026-11-09

### Thay đổi quan trọng
- `rustype-linux` thay thế engine IBus và addon Fcitx5. Đây là một tiến trình duy nhất, gồm bus IBus và engine, và là đường cài đặt mặc định trên Linux. `./install.sh` không cần sudo và tự khôi phục ibus nếu có lỗi. `./uninstall.sh` quay về ibus-daemon.
- Linux chỉ hỗ trợ GNOME Shell trên Wayland. Ứng dụng Flatpak, KDE, Sway, Hyprland, engine IBus khác và emoji panel chưa được hỗ trợ.
- Chế độ `mode = direct` không còn gửi phím BackSpace giả. Ứng dụng không có Surrounding Text dùng preedit.
- Gói `.deb` dùng được ngay sau khi cài, không cần đăng xuất: postinst chạy `rustype-linux --activate --restart` cho người dùng đang đăng nhập, và autostart kích hoạt cho các lần đăng nhập sau. `rustype-linux --activate` thêm Rustype vào nguồn nhập GNOME một lần, và `--deactivate` bỏ đi. Khi gỡ gói, prerm bỏ Rustype khỏi nguồn nhập và postrm khởi động lại IBus.

### Sửa lỗi
- VNI gõ đúng các từ có `ươ` sau `h`, `kh`, `th`, như "hương", "thường", "thước", "khương". Telex `huouw` và `khuouw` ra "hươu" và "khươu".
- Gõ được các âm tiết từng bị coi là từ tiếng Anh: "môn", "nôn", "bể", "bổ", "mổ", "rôm", "sên", "têm". Vì vậy `moon`, `noon`, `beer`, `boor`, `moor`, `room`, `seen`, `teem` giờ ra tiếng Việt; muốn giữ tiếng Anh thì thêm vào danh sách từ riêng.
- Không còn mất bộ gõ khi đăng nhập sau mất điện hoặc crash: file địa chỉ cũ có PID trùng một tiến trình khác không còn chặn `rustype-linux` khởi động.
- Ứng dụng Cài đặt: từ gõ tắt và từ riêng đã xóa không còn quay lại.
- Thanh địa chỉ Chrome: ô URL dùng Surrounding Text không gạch chân, giữ được gợi ý và phím Enter. Khi Chrome bỏ qua lệnh xóa vì đang chọn gợi ý, engine xóa phần đuôi rồi commit lại từ.
- Backspace mở lại được từ vừa bị đóng băng để sửa tiếp.
- Gõ `uu` rồi phím móc `w` đặt dấu móc lên u đầu: `uuw` ra "ưu", `Luuw` ra "Lưu".

### Thêm
- Test tự sinh âm tiết tiếng Việt và gõ thử bằng cả Telex và VNI (`rustype-core/tests/syllables.rs`).
- Tài liệu: kiến trúc, phát triển, xử lý sự cố, lộ trình, bảng tương thích ứng dụng (`docs/COMPATIBILITY.md`).
- File giấy phép MIT cho Rustype.
- Bộ kết quả chuẩn `rustype-core/tests/golden.tsv` gồm 14.710 chuỗi phím, kiểm tra trong `cargo test`, và công cụ `tools/golden.py` để xem và ghi nhận khác biệt khi đổi luật gõ.
- Cargo workspace với `rustype-core` (lõi Telex thuần, không phụ thuộc nền tảng).
- CI GitHub Actions: kiểm tra định dạng, clippy, unit test, golden test, IBus giả lập.
- Fuzz target cho lõi Telex bằng `cargo-fuzz`.
- Xử lý `SetSurroundingText` và selection autocomplete cho thanh địa chỉ Chrome / Firefox / Electron.
- Hỗ trợ cấu hình chế độ hiển thị riêng cho từng ứng dụng qua mục `[app_modes]` trong `config`.
- Interface D-Bus `SetFocusApp` và GNOME Shell extension `rustype-focus@namtr.dev` để nhận diện app focus trên Wayland.
- C FFI cho `rustype-core` (`rustype.h`, thư viện tĩnh `librustype_core.a` và động `librustype_core.so`).
- Kiểu gõ VNI (`input_method = vni`) với luật gõ số và phím lặp thoát dấu.
- Bảng gõ tắt (Macro) tương thích định dạng file Unikey (`~/.config/rustype/macro` hoặc mục `[macros]`).
- Danh sách từ tiếng Anh người dùng tự thêm (`custom_words` hoặc file `~/.config/rustype/words`).
- Tính năng nhớ trạng thái bật/tắt tiếng Việt theo từng ứng dụng (`remember_state_per_app = true`, giống EVKey).
- Lệnh chẩn đoán `rustype-linux --doctor`.
- Menu IBus Panel Properties: bật/tắt tiếng Việt, đổi kiểu gõ và mở Cài đặt trực tiếp từ thanh trạng thái.
- Ứng dụng cài đặt đồ họa native GNOME GTK4 + Libadwaita (`rustype-settings`) kèm launcher `.desktop`.

### Đổi
- Chế độ gỡ lỗi (`RUSTYPE_DEBUG`) chỉ in mã phím, không in chữ người dùng gõ.

### Bỏ
- Công cụ so sánh với engine bên ngoài và công cụ sinh bảng luật. `rustype-core/src/tables.rs` giờ được sửa trực tiếp.
- Chế độ hiện chữ bằng phím BackSpace giả của `mode = direct`. Không hoạt động trong Chrome và VTE trên GNOME Wayland.
- Đường cài đặt `rustype-ibus` và `rustype-fcitx5`, cùng `ibus-engine-rustype --doctor`. Thay bằng `rustype-linux`, và `rustype-linux --doctor`.

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

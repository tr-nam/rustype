# Lộ trình

Mục tiêu: biến Rustype thành một bộ gõ tiếng Việt hoàn chỉnh cho Linux. Trên Linux, phạm vi là GNOME Shell trên Wayland. Windows và macOS đã có bản riêng.

Một bộ gõ hoàn chỉnh cần bốn thứ:

1. Gõ đúng trên các ứng dụng thường dùng trong GNOME.
2. Có giao diện để bật tắt và cài đặt.
3. Cài bằng một gói hoặc một script, không cần Rust.
4. Có quy trình phát hành và hỗ trợ người dùng.

## Mặc định hiện nay: rustype-linux

- [x] `rustype-linux` là đường cài đặt mặc định trên Linux. Một tiến trình duy nhất, thay cho `ibus-daemon` và `ibus-engine-rustype`.
- [x] Không còn `rustype-ibus` và `rustype-fcitx5` như hai đường riêng.
- [x] `./install.sh` không cần sudo, tự kiểm tra và tự khôi phục ibus nếu có lỗi. `./uninstall.sh` quay về ibus-daemon.
- [x] Chạy thử trên một máy Ubuntu GNOME Wayland thật.
- [x] Chỉ hỗ trợ GNOME Shell trên Wayland. Xem [COMPATIBILITY.md](COMPATIBILITY.md).

## Đã có trong 0.1.0

- [x] Lõi Telex có bộ kết quả chuẩn 14.710 chuỗi phím, kiểm tra trong `cargo test`. Sửa luật có chủ đích, rồi duyệt khác biệt bằng `tools/golden.py`.
- [x] Engine không dùng thư viện ngoài: binary `rustype-linux` khoảng 755 KB, dùng khoảng 3 MB RAM.
- [x] Không lặp chữ trong terminal trên GNOME Wayland.
- [x] Không mất chữ khi bấm chuột hoặc đổi cửa sổ lúc đang gõ dở.
- [x] Không gạch chân ở ứng dụng có Surrounding Text.
- [x] Bỏ qua ô mật khẩu.
- [x] Kiểm thử: unit test, bộ kết quả chuẩn, sandbox IBus với client libibus thật.

## Giai đoạn 0: Nền móng

- [x] Đưa code lên git và GitHub.
- [x] Tách lõi Telex thành crate riêng, không phụ thuộc nền tảng. Các nền tảng khác dùng lại qua C FFI.
- [x] CI chạy kiểm tra định dạng, clippy, unit test, bộ kết quả chuẩn và sandbox cho mỗi lần push.
- [x] Fuzz phần đọc D-Bus và lõi Telex. Bộ gõ nhận mọi phím, kể cả mật khẩu, nên không được sập.
- [x] Chế độ gỡ lỗi chỉ in mã phím, không in chữ người dùng gõ.

## Giai đoạn 1: Gõ đúng ở mọi nơi trên GNOME Wayland

Với một bộ gõ, gõ đúng ở mọi nơi quan trọng hơn có thêm tính năng.

- [x] Thử thật trên từng ứng dụng và ghi kết quả vào một bảng: xem [COMPATIBILITY.md](COMPATIBILITY.md).
- [x] Thanh địa chỉ có gợi ý tự động trong Chrome và Electron: ô URL dùng Surrounding Text không gạch chân, giữ được gợi ý và phím Enter. Chỉ ở ô URL, engine yêu cầu Surrounding Text, và khi Chrome bỏ qua lệnh xóa thì xóa phần đuôi rồi commit lại từ.
- [x] GNOME Shell extension báo tên ứng dụng đang focus cho engine (`gnome-shell-extension/rustype-focus@namtr.dev`).
- [x] Cách xử lý riêng cho từng ứng dụng: mục `[app_modes]` trong `~/.config/rustype/config`.
- [x] Chế độ `direct` không còn dùng phím Backspace giả. Ứng dụng không có Surrounding Text dùng preedit.
- [ ] Thử nghiệm thêm trên các phiên bản GNOME và máy khác nhau.

Ngoài phạm vi, không có kế hoạch: KDE Plasma, Sway và Hyprland (kể cả với Fcitx5), Flatpak, và các engine IBus khác.

## Giai đoạn 2: Đủ tính năng như Unikey

- [x] Nhớ trạng thái Việt hay Anh theo từng ứng dụng, như EVKey (`remember_state_per_app = true`).
- [x] Kiểu gõ VNI (`input_method = vni`) với luật gõ số và escape chuẩn Unikey.
- [x] Gõ tắt (macro) theo định dạng chuẩn Unikey (`~/.config/rustype/macro` hoặc section `[macros]`).
- [x] Danh sách từ tiếng Anh người dùng tự thêm (`custom_words` trong config hoặc file `~/.config/rustype/words`).
- [ ] Phím tắt bật tắt tiếng Việt ngay trong bộ gõ, ví dụ Ctrl+Shift, không phải đổi nguồn nhập.
- [ ] Kiểu gõ VIQR.
- [ ] Tùy chọn gõ tự do, Quick Telex, bật tắt kiểm tra chính tả.
- [ ] Bảng mã cũ như TCVN3, VNI-Windows và công cụ chuyển mã clipboard. Ưu tiên thấp, chỉ làm khi có người cần.

## Giai đoạn 3: Giao diện

- [x] Menu trong biểu tượng bộ gõ trên thanh GNOME: bật/tắt tiếng Việt (switch toggle), đổi kiểu gõ (Telex/VNI) và mở Cài đặt trực tiếp từ menu thuộc tính.
- [x] App cài đặt bằng GTK4 và Libadwaita (`tools/rustype-settings`), độc lập giúp engine siêu nhẹ.
- [x] Lệnh chẩn đoán: `rustype-linux --doctor` kiểm tra bus, file đăng ký, nguồn nhập, extension và cấu hình.
- [x] Giao diện hỗ trợ song ngữ Tiếng Việt và Tiếng Anh.

## Giai đoạn 4: Đóng gói và phát hành

- [x] Gói .deb: engine và drop-in systemd của hệ thống, có man page (`docs/man/`), kèm `LICENSE`.
- [x] Kênh phát hành: cấu hình gói Debian/Ubuntu (`packaging/debian`, `packaging/build-deb.sh`), AUR cho Arch (`packaging/arch/PKGBUILD`), Fedora RPM (`packaging/fedora/rustype.spec`), GitHub Releases có checksum SHA256 (`.github/workflows/release.yml`).
- [x] Không làm Flatpak hay Snap, vì bộ gõ IBus phải được cài vào hệ thống.
- [x] Cập nhật qua trình quản lý gói. Engine không có mã mạng, cam kết quyền riêng tư tại [PRIVACY.md](PRIVACY.md).

## Giai đoạn 5: Windows và macOS

- [x] Nâng cấp `rustype-core` C FFI (`include/rustype.h`): hỗ trợ Telex, VNI, Macro, Custom Words và Preedit qua C ABI.
- [x] Windows: crate `rustype-windows` theo cơ chế Low-Level Keyboard Hook (`SetWindowsHookExW` + `SendInput`) và khay hệ thống (System Tray `Shell_NotifyIconW`).
- [x] macOS: cấu trúc Input Method bundle `rustype-macos` (`Rustype.app`), lớp vỏ InputMethodKit liên kết tĩnh với `librustype_core.a`.
- [x] Ma trận kiểm thử CI/CD đa nền tảng trên GitHub Actions cho Linux, Windows (`windows-latest`) và macOS (`macos-latest`).
- [ ] Chứng chỉ ký code (Code Signing) cho Windows Authenticode và Apple Developer ID (Notarization).

## Giai đoạn 6: Ra mắt

- [x] Trang web và hướng dẫn tiếng Việt, câu hỏi thường gặp (`website/`).
- [x] Mẫu báo lỗi chuẩn GitHub, yêu cầu ghi ứng dụng, desktop, Wayland hay X11, và kết quả `rustype-linux --doctor` (`.github/ISSUE_TEMPLATE/*`).
- [x] Trang cam kết quyền riêng tư (`docs/PRIVACY.md` và trên website).
- [x] Kiểm tra tên và thương hiệu trước khi làm web: repo cá nhân `tr-nam/rustype`, hỗ trợ qua `support@namtr.dev`.
- [x] Triển khai website trên VPS.

## Quyết định cần chốt

| Câu hỏi | Lựa chọn | Đề xuất | Khi nào cần chốt |
|---|---|---|---|
| Phạm vi Linux | GNOME Shell trên Wayland, hay nhiều desktop | Chỉ GNOME Shell trên Wayland | Đã chốt |
| Bộ công cụ giao diện | GTK4, Qt, hay giao diện web | GTK4, tự nhiên trên GNOME và không làm phình engine | Đã chốt cho Linux |
| Hướng làm cho Windows | Text Services Framework, hay bắt phím như Unikey | Chưa đề xuất, cần thử cả hai | Trước khi làm lại Windows |

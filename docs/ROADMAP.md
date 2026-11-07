# Lộ trình

Mục tiêu: biến Rustype thành một bộ gõ hoàn chỉnh như Unikey. Bản 1.0 chỉ cho Linux. Windows và macOS để bản 2.

Một bộ gõ hoàn chỉnh cần bốn thứ mà bản 0.1 chưa có:

1. Gõ đúng trên nhiều ứng dụng và nhiều desktop, không chỉ GNOME của người viết.
2. Có giao diện để bật tắt và cài đặt.
3. Cài bằng một gói, không cần Rust hay script sudo.
4. Có quy trình phát hành và hỗ trợ người dùng.

## Đã có trong 0.1.0

- [x] Lõi Telex có bộ kết quả chuẩn 14.710 chuỗi phím, kiểm tra trong `cargo test`. Sửa luật có chủ đích, ví dụ gõ ra "thuở", rồi duyệt khác biệt bằng `tools/golden.py`.
- [x] Engine IBus không dùng thư viện ngoài: khoảng 450 KB, khoảng 2,5 MB RAM.
- [x] Không lặp chữ trong terminal trên GNOME Wayland.
- [x] Không mất chữ khi bấm chuột hoặc đổi cửa sổ lúc đang gõ dở.
- [x] Không gạch chân ở ứng dụng có Surrounding Text.
- [x] Bỏ qua ô mật khẩu.
- [x] Kiểm thử: unit test, bộ kết quả chuẩn, IBus giả lập.

## Giai đoạn 0: Nền móng

- [x] Đưa code lên git và GitHub.
- [x] Tách lõi Telex thành crate riêng, không phụ thuộc nền tảng. Lớp IBus dùng crate này, sau này Fcitx5, Windows và macOS cũng vậy.
- [x] CI chạy kiểm tra định dạng, clippy, unit test, bộ kết quả chuẩn và IBus giả lập cho mỗi lần push.
- [x] Fuzz phần đọc D-Bus và lõi Telex. Bộ gõ nhận mọi phím, kể cả mật khẩu, nên không được sập.
- [x] Chế độ gỡ lỗi chỉ in mã phím, không in chữ người dùng gõ.

## Giai đoạn 1: Gõ đúng ở mọi nơi trên Linux

Đây là giai đoạn quan trọng nhất. Với một bộ gõ, gõ đúng ở mọi nơi quan trọng hơn có thêm tính năng.

- [x] Thử thật trên từng ứng dụng và ghi kết quả vào một bảng: xem [COMPATIBILITY.md](COMPATIBILITY.md).
- [x] Xử lý thanh địa chỉ có gợi ý tự động trong Chrome và Electron: bắt `SetSurroundingText`, kiểm tra selection và cursor jump.
- [x] Bản cho Fcitx5: C FFI trong `rustype-core` (`rustype.h`, `librustype_core.a`) và template addon C++ `rustype-fcitx5/`.
- [x] GNOME Shell extension báo tên ứng dụng đang focus cho engine (`gnome-shell-extension/rustype-focus@namtr.dev`).
- [x] Cách xử lý riêng cho từng ứng dụng: hỗ trợ mục `[app_modes]` trong `~/.config/rustype/config`.
- [ ] Thử nghiệm mở rộng trên các desktop khác (KDE Plasma, Sway, Hyprland với Fcitx5).

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

- [ ] Menu trong biểu tượng bộ gõ trên thanh GNOME: bật/tắt tiếng Việt, đổi kiểu gõ trực tiếp từ thanh trạng thái (Hiện tại trên GNOME Shell cần chuyển nguồn nhập qua Super+Space; việc đổi kiểu gõ Telex/VNI và cấu hình được thực hiện qua ứng dụng Cài đặt Rustype hoặc file config; engine đã có mã giao thức IBus Property nhưng GNOME Shell mặc định không hiển thị menu con này).
- [x] App cài đặt bằng GTK4 và Libadwaita (`tools/rustype-settings`), độc lập giúp engine siêu nhẹ.
- [x] Lệnh chẩn đoán: `ibus-engine-rustype --doctor` kiểm tra IBus, daemon, component XML, nguồn nhập, extension và cấu hình.
- [x] Giao diện hỗ trợ song ngữ Tiếng Việt và Tiếng Anh.

## Giai đoạn 4: Đóng gói và phát hành

- [x] Gói .deb chuẩn: engine vào `/usr/libexec`, file đăng ký vào `/usr/share/ibus/component`, có man page (`docs/man/`), kèm `LICENSE`.
- [x] Kênh phát hành: cấu hình gói Debian/Ubuntu (`packaging/debian`, `packaging/build-deb.sh`), AUR cho Arch (`packaging/arch/PKGBUILD`), Fedora RPM (`packaging/fedora/rustype.spec`), GitHub Releases có checksum SHA256 (`.github/workflows/release.yml`).
- [x] Không làm Flatpak hay Snap, vì bộ gõ IBus phải được cài vào hệ thống.
- [x] Cập nhật qua trình quản lý gói. Engine không kết nối mạng, cam kết quyền riêng tư tại [PRIVACY.md](PRIVACY.md).

Vì không dùng thư viện ngoài, Rustype đóng gói cho Debian và PPA dễ hơn hầu hết dự án Rust khác.

## Giai đoạn 5: Windows và macOS, cho bản 2

- [ ] Windows, chọn một trong hai hướng:
  - Text Services Framework: cách chuẩn, có chữ gạch chân như bộ gõ thật, nhưng khó làm.
  - Bắt phím và gửi phím xóa giả như Unikey: dễ hơn, chạy cả trong game, nhưng hay bị phần mềm diệt virus nghi ngờ.
- [ ] macOS: InputMethodKit, lớp vỏ Swift gọi lõi Rust.
- [ ] Chứng chỉ ký code cho cả hai nền tảng, nếu không người dùng gặp cảnh báo bảo mật khi cài. Có phí hằng năm.

## Giai đoạn 6: Ra mắt

- [ ] Trang web và hướng dẫn tiếng Việt, câu hỏi thường gặp về terminal, Wayland và Chrome.
- [ ] Mẫu báo lỗi bắt buộc ghi ứng dụng, desktop, Wayland hay X11, và kết quả lệnh chẩn đoán.
- [ ] Trang cam kết quyền riêng tư: không mạng, không ghi phím.
- [ ] Kiểm tra tên trước khi làm web. Tên `rustype` còn trống trên crates.io vào ngày 2026-11-07. Trên GitHub, tên tổ chức `rustype` đã có người dùng cho một dự án Rust khác, nên repo cần đặt dưới tài khoản cá nhân.

## Ước tính thời gian

Ước tính cho một người làm bán thời gian, chỉ để tham khảo.

| Giai đoạn | Kết quả | Thời gian |
|---|---|---|
| 0. Nền móng | Repo, CI, lõi tách riêng | 1 tuần |
| 1. Gõ đúng mọi nơi | Bảng thử ứng dụng xong, có bản Fcitx5 | 2 đến 4 tuần |
| 2. Đủ tính năng | Phím tắt, VNI, gõ tắt, nhớ theo ứng dụng | 3 đến 5 tuần |
| 3. Giao diện | App cài đặt (GTK4/Adwaita), lệnh chẩn đoán | 2 đến 3 tuần |
| 4. Đóng gói | .deb, PPA, AUR | 1 đến 2 tuần |
| **Bản 1.0 cho Linux** | | **khoảng 2 đến 3 tháng** |
| 5. Windows và macOS | Bản 2 | nhiều tháng |

## Quyết định cần chốt

| Câu hỏi | Lựa chọn | Đề xuất | Khi nào cần chốt |
|---|---|---|---|
| Phạm vi bản 1.0 | Chỉ Linux, hay đa nền tảng ngay | Chỉ Linux | Trước giai đoạn 0 |
| Bộ công cụ giao diện | GTK4, Qt, hay giao diện web | GTK4, tự nhiên trên GNOME và không làm phình engine | Trước giai đoạn 3 |
| Hướng làm cho Windows | Text Services Framework, hay bắt phím như Unikey | Chưa đề xuất, cần thử cả hai | Trước giai đoạn 5 |

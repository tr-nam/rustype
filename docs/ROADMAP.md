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

- [ ] Đưa code lên git và GitHub.
- [ ] Tách lõi Telex thành crate riêng, không phụ thuộc nền tảng. Lớp IBus dùng crate này, sau này Fcitx5, Windows và macOS cũng vậy.
- [ ] CI chạy kiểm tra định dạng, clippy, unit test, bộ kết quả chuẩn và IBus giả lập cho mỗi lần push.
- [ ] Fuzz phần đọc D-Bus và lõi Telex. Bộ gõ nhận mọi phím, kể cả mật khẩu, nên không được sập.
- [ ] Chế độ gỡ lỗi chỉ in mã phím, không in chữ người dùng gõ.

## Giai đoạn 1: Gõ đúng ở mọi nơi trên Linux

Đây là giai đoạn quan trọng nhất. Với một bộ gõ, gõ đúng ở mọi nơi quan trọng hơn có thêm tính năng.

- [ ] Thử thật trên từng ứng dụng và ghi kết quả vào một bảng:

  | Nhóm | Ứng dụng |
  |---|---|
  | Terminal | Ptyxis, GNOME Terminal, Konsole, kitty, Alacritty, WezTerm |
  | Trình duyệt | Chrome, Firefox, đặc biệt là thanh địa chỉ |
  | Electron | VS Code, Obsidian, Slack, Discord |
  | Khác | LibreOffice, Telegram, IDE JetBrains, ứng dụng Flatpak |

- [ ] Thử trên từng desktop: GNOME Wayland, GNOME X11, KDE Plasma, Xfce, Sway, Hyprland.
- [ ] Xử lý thanh địa chỉ có gợi ý tự động trong Chrome và Electron. Bộ gõ sửa chữ bằng Surrounding Text hay gặp lỗi ở chỗ này.
- [ ] Bản cho Fcitx5: một addon C++ mỏng gọi lõi Rust. Người dùng KDE và Hyprland phần lớn dùng Fcitx5 thay vì IBus.
- [ ] GNOME Shell extension báo tên ứng dụng đang focus cho engine. Trên GNOME Wayland, mọi ứng dụng dùng chung một kết nối IBus tên "gnome-shell", nên engine không tự biết đang gõ vào đâu. Đây là điều kiện cho các tính năng theo từng ứng dụng.
- [ ] Cách xử lý riêng cho từng ứng dụng, ví dụ ép dùng gạch chân ở ứng dụng nào sửa chữ sai.

## Giai đoạn 2: Đủ tính năng như Unikey

- [ ] Phím tắt bật tắt tiếng Việt ngay trong bộ gõ, ví dụ Ctrl+Shift, không phải đổi nguồn nhập.
- [ ] Nhớ trạng thái Việt hay Anh theo từng ứng dụng, như EVKey. Cần extension ở giai đoạn 1.
- [ ] Kiểu gõ VNI và VIQR.
- [ ] Tùy chọn gõ tự do, Quick Telex, bật tắt kiểm tra chính tả.
- [ ] Gõ tắt, nhập và xuất được file theo định dạng của Unikey.
- [ ] Danh sách từ tiếng Anh người dùng tự thêm, cho các từ như "this" đang bị thành "thí".
- [ ] Bảng mã cũ như TCVN3, VNI-Windows và công cụ chuyển mã clipboard. Ưu tiên thấp, chỉ làm khi có người cần.

## Giai đoạn 3: Giao diện

- [ ] Menu trong biểu tượng bộ gõ trên thanh GNOME: bật tắt, đổi kiểu gõ, mở cài đặt. IBus có sẵn cơ chế cho menu này.
- [ ] App cài đặt bằng GTK4 và libadwaita, là chương trình riêng để engine vẫn nhẹ. Hai bên dùng chung file cấu hình.
- [ ] Lệnh chẩn đoán: kiểm tra IBus, file đăng ký, nguồn nhập, biến môi trường, và ứng dụng đang dùng có Surrounding Text không.
- [ ] Giao diện tiếng Việt và tiếng Anh.

## Giai đoạn 4: Đóng gói và phát hành

- [ ] Gói .deb chuẩn: engine vào `/usr/libexec`, file đăng ký vào `/usr/share/ibus/component`, có man page, kèm `LICENSE`.
- [ ] Kênh phát hành: PPA cho Ubuntu, AUR cho Arch, COPR cho Fedora, GitHub Releases có checksum và chữ ký.
- [ ] Không làm Flatpak hay Snap, vì bộ gõ IBus phải được cài vào hệ thống.
- [ ] Cập nhật qua trình quản lý gói. Engine không kết nối mạng, ghi rõ điều này như một cam kết về quyền riêng tư.

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
| 3. Giao diện | Menu, app cài đặt, lệnh chẩn đoán | 2 đến 3 tuần |
| 4. Đóng gói | .deb, PPA, AUR | 1 đến 2 tuần |
| **Bản 1.0 cho Linux** | | **khoảng 2 đến 3 tháng** |
| 5. Windows và macOS | Bản 2 | nhiều tháng |

## Quyết định cần chốt

| Câu hỏi | Lựa chọn | Đề xuất | Khi nào cần chốt |
|---|---|---|---|
| Phạm vi bản 1.0 | Chỉ Linux, hay đa nền tảng ngay | Chỉ Linux | Trước giai đoạn 0 |
| Bộ công cụ giao diện | GTK4, Qt, hay giao diện web | GTK4, tự nhiên trên GNOME và không làm phình engine | Trước giai đoạn 3 |
| Hướng làm cho Windows | Text Services Framework, hay bắt phím như Unikey | Chưa đề xuất, cần thử cả hai | Trước giai đoạn 5 |

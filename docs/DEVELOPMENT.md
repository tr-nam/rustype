# Phát triển

## Cần có

- Rust stable, cài bằng [rustup](https://rustup.rs). Engine không dùng crate ngoài nào, nên build được cả khi không có mạng.
- Để chạy công cụ kiểm thử: `python3`, `python3-gi`, `gir1.2-ibus-1.0`, `ibus` và `dbus-run-session`. Trên Ubuntu các gói này thường có sẵn khi đã cài IBus.

## Build và cài thử

```
cargo build --release          # tạo target/release/ibus-engine-rustype
./install.sh                   # build, cài vào ~/.local/lib/rustype, đăng ký với IBus
```

`install.sh` làm lần lượt:

1. Build bản release.
2. Chép engine vào `~/.local/lib/rustype/ibus-engine-rustype`.
3. Tạo `~/.config/rustype/config` nếu chưa có.
4. Ghi file component vào `/usr/share/ibus/component/rustype.xml`. Chỉ bước này cần sudo, và chỉ chạy khi nội dung file thay đổi.
5. Chạy `ibus restart` để IBus nạp engine mới.
6. Thêm Rustype vào danh sách bộ gõ của GNOME nếu chưa có.

Sau mỗi lần sửa code, chạy lại `./install.sh`. Lúc đó thường không cần sudo nữa.

## Kiểm thử

Có hai lệnh, nên chạy cả hai trước khi phát hành:

```
cargo test --release
tools/ibus_sandbox_test.sh "$PWD/target/release/ibus-engine-rustype" <(target/release/ibus-engine-rustype --xml) rustype
```

| Mức | Kiểm tra gì | Kết quả mong đợi |
|---|---|---|
| Unit test | Từng module: ghép chữ, FFI, chọn cách hiện chữ, đóng gói D-Bus, đọc cấu hình, chẩn đoán | Mọi crate đều báo `test result: ok`, không có test nào fail |
| Bộ kết quả chuẩn | Nằm trong `cargo test`. Các chuỗi phím trong `rustype-core/tests/golden.tsv`: câu mẫu, âm tiết với nhiều kiểu gõ, từ tiếng Anh, chuỗi ngẫu nhiên có BackSpace | Không có trường hợp nào đổi |
| IBus giả lập | Một ibus-daemon riêng và các ứng dụng giả: terminal, ứng dụng GUI, ô thanh địa chỉ (URL), ứng dụng X11. Kiểm tra bấm chuột, Enter, BackSpace, đổi cửa sổ | `ALL PASS` |

Kiểm thử IBus giả lập chạy trong `dbus-run-session` với địa chỉ, cấu hình và bộ nhớ đệm riêng. Nó không đụng tới IBus đang chạy trên máy.

**Thử nhanh một chuỗi phím** mà không cần IBus:

```
echo "Vieetj Nam ddepj" | target/release/ibus-engine-rustype --feed
```

## Đổi luật gõ

Luật nằm trong `rustype-core/src/tables.rs` và `rustype-core/src/telex.rs`, sửa trực tiếp. Sau khi sửa, `cargo test` sẽ báo những chuỗi phím có kết quả khác trước. Quy trình:

1. Build lại: `cargo build --release`.
2. Xem mọi trường hợp bị đổi: `python3 -I tools/golden.py diff`.
3. Nếu mọi thay đổi đều đúng ý, ghi nhận chúng: `python3 -I tools/golden.py update`.
4. Ghi thay đổi vào [CHANGELOG.md](../CHANGELOG.md).

Muốn tạo lại toàn bộ chuỗi phím đầu vào, ví dụ khi thêm loại trường hợp mới vào `tools/golden.py`, dùng `python3 -I tools/golden.py update --new`.

## Gỡ lỗi trên máy thật

Biến `RUSTYPE_DEBUG=1` in mã từng phím ra stderr (chỉ mã phím, không in chữ người dùng gõ). Vì engine do ibus-daemon khởi chạy, muốn xem log thì phải tự chạy nó:

```
pkill -x ibus-engine-rus
RUSTYPE_DEBUG=1 ~/.local/lib/rustype/ibus-engine-rustype --ibus
```

Tên tiến trình bị Linux cắt còn 15 ký tự, nên lệnh `pkill` dùng `ibus-engine-rus`.

Sau khi chạy lệnh thứ hai, nhấn Super+Space để chuyển sang bộ gõ khác rồi chuyển lại Rustype. IBus chỉ dùng engine vừa chạy sau lần chuyển đó. Gỡ lỗi xong thì nhấn Ctrl+C và chạy `ibus restart`.

> Log chứa mã phím, có thể suy ra nội dung gõ. Đừng gửi log cho người khác nếu chưa đọc lại.

## Fuzz

Fuzz lõi Telex bằng `cargo-fuzz` (cần cài riêng: `cargo install cargo-fuzz`):

```
cd fuzz
cargo fuzz run fuzz_telex -- -max_total_time=300
```

Fuzz chạy không giới hạn nếu không đặt `max_total_time`. Crash được lưu vào `fuzz/artifacts/`.

## Đóng gói và Phát hành

Quy trình đóng gói đã được tự động hóa qua `Makefile` và GitHub Actions:

1. **Đóng gói .deb cục bộ**:
   ```bash
   make deb
   ```
   Tạo file `.deb` chuẩn FHS tại `target/packages/rustype_*.deb`.

2. **Quy trình phát hành bản mới**:
   - Kiểm tra mã nguồn: `make check`.
   - Cập nhật `version` trong `Cargo.toml`, `packaging/debian/changelog`.
   - Ghi chú thay đổi vào [CHANGELOG.md](../CHANGELOG.md).
   - Đẩy tag lên GitHub (ví dụ: `git tag v0.1.0 && git push origin v0.1.0`). GitHub Actions sẽ tự động biên dịch, sinh file `.deb`, tarball, tính checksum SHA256 và đăng tải bản phát hành.

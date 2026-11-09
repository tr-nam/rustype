# Phát triển

## Cần có

- Rust stable, cài bằng [rustup](https://rustup.rs). Crate `rustype-linux` và `rustype-core` chỉ dùng thư viện chuẩn, nên build được cả khi không có mạng.
- Để chạy sandbox và thử với client libibus thật: `python3`, `python3-gi`, `gir1.2-ibus-1.0`, `ibus` và `dbus-run-session`. Trên Ubuntu các gói này thường có sẵn khi đã cài IBus.

## Build và cài thử

```
cargo build --release          # tạo target/release/rustype-linux
./install.sh                   # build và đưa rustype-linux vào phiên GNOME (không cần sudo)
```

`install.sh` tự kiểm tra kết quả và tự khôi phục ibus nếu có lỗi. Để quay về ibus-daemon: `./uninstall.sh`. Chi tiết về drop-in systemd nằm trong [ARCHITECTURE.md](ARCHITECTURE.md#tiếp-quản-dịch-vụ-ibus-trên-gnome).

Sau mỗi lần sửa code, chạy lại `cargo build --release` rồi `./install.sh`.

## Kiểm thử

```
cargo test
tools/rustype_linux_sandbox.sh target/release/rustype-linux
```

| Mức | Kiểm tra gì | Kết quả mong đợi |
|---|---|---|
| Unit test | Từng module: ghép chữ, FFI, chọn cách hiện chữ, đóng gói D-Bus, đọc cấu hình, chẩn đoán | `cargo test` kết thúc với `test result: ok` ở mọi crate |
| Bộ kết quả chuẩn | `rustype-core/tests/golden.tsv`, chạy trong `cargo test`: câu mẫu, âm tiết với nhiều kiểu gõ, từ tiếng Anh, chuỗi ngẫu nhiên có BackSpace | Không có trường hợp nào đổi, trừ khi đã cố ý đổi luật (xem dưới) |
| Mô hình Omnibox | `rustype-linux/tests/omnibox_model.rs`, chạy bộ kết quả chuẩn qua một mô hình của ô thanh địa chỉ Chrome | `cargo test` không báo lỗi |
| Sandbox | `tools/rustype_linux_sandbox.sh`: bus riêng trong `dbus-run-session`, client libibus thật, panel, và kiểm tra dọn dẹp khi nhận SIGTERM | `ALL PASS` hoặc không có lỗi nào |

Sandbox chạy với địa chỉ, cấu hình và bộ nhớ đệm riêng. Nó không đụng tới IBus đang chạy trên máy.

### Kiểm thử tích hợp (install.sh, uninstall.sh, gói .deb)

```
make test-integration                 # build rồi chạy tests/run_all.sh
RUSTYPE_LINUX_BIN=/abs/path/rustype-linux tests/run_all.sh
```

Bộ kiểm thử nằm trong `tests/`: `install/run.sh` (cài đặt, gỡ cài đặt), `deb/run.sh` (gói .deb và các script bảo trì), `settings/` (cài đặt ứng dụng). Mỗi test chạy trong một sandbox riêng: HOME và XDG_* tạm, `env -i`, PATH chỉ có các stub và vài công cụ vô hại, không có `sudo`, `systemctl`, `gsettings` thật. Lệnh không có sẵn trong sandbox làm test thất bại. Bộ này không cần quyền root, phiên GNOME hay GNOME đã cài. `dpkg -i` không chạy được trong sandbox.

Kết quả kết thúc bằng một dòng `SUMMARY: passed=… failed=… known-fail=…`. Test `known-fail` ghi lại một lỗi đã biết của sản phẩm: nó được báo `KNOWN-FAIL` và không làm hỏng bộ kiểm thử. Nếu nó bất ngờ pass, bộ kiểm thử báo `UNEXPECTED-PASS` và thất bại, để gỡ đánh dấu.

Chỉ chạy một phần: `RT_FILTER=<chuỗi> tests/install/run.sh`.

**Thử nhanh một chuỗi phím** mà không cần IBus:

```
echo "Vieetj Nam ddepj" | target/release/rustype-linux --feed
```

### Kiểm thử đầu cuối (GNOME Shell thật, headless)

`make test-e2e` chạy rustype-linux với GNOME Shell 50 headless trên máy này, trong một phiên D-Bus và một D-Bus hệ thống riêng. Mỗi ca chạy trong một phiên mới, có `timeout 150`, và chỉ dừng các nhóm tiến trình do chính nó tạo. Trước và sau khi chạy, bộ kiểm tra so sánh trạng thái `org.freedesktop.IBus.session.GNOME.service` và `org.gnome.desktop.input-sources sources` của phiên thật: hai giá trị phải không đổi.

`make test-e2e-46` chạy cùng bộ ca trong Docker, trên Ubuntu 24.04 với GNOME 46. Ảnh được build từ cây làm việc, và rustype-linux được biên dịch ngay trong ảnh, không sao chép từ máy host. Container chạy với `--network none`, không đặc quyền, không gắn kết thư mục từ máy host. Ảnh và container đều có nhãn `rustype-e2e=1`.

| Ca | Nội dung | Kết quả mong đợi |
|---|---|---|
| a | Terminal VTE thật, `tieengs vieejt` + Enter vào `cat` | `tiếng việt`, Shell còn sống |
| b | Ô GTK, ứng dụng tự xóa ô giữa chừng, không có phím nào: `vieej` xóa `tr`; `tieengs` xóa `Traanf` | `tr`, Shell và ứng dụng còn sống; ca thứ hai là lỗi đã biết |
| c | Chuyển focus từ ô GTK có chữ sang terminal trong cùng cửa sổ | `tiếng` và `việt` |
| d | Telex cả câu, chữ hoa, Backspace, ô mật khẩu giữ nguyên, ô thường không gạch chân kể cả sau khi đổi focus | `Tiếng Việt`, `Tiếng Việ`, `tieengs`, 0 lần preedit |
| e | Chrome: thanh địa chỉ có gợi ý nội tuyến chọn sẵn, và một ô `<input>` | `Rú`, `Trần` |

Ca b thứ hai được ghi là `KNOWN-FAIL`: GTK chỉ báo nội dung ô sau phím kế tiếp, nên nếu phím đó làm thay chữ của từ cũ thì engine chưa kịp biết ô đã bị xóa. Nếu ca này bất ngờ đạt, bộ kiểm tra báo `UNEXPECTED-PASS` và thất bại, để gỡ nhãn. Ca e cần Google Chrome; ảnh GNOME 46 không có Chrome nên ca bị bỏ qua và được ghi là `SKIP`. Bộ kiểm tra không dùng `gsettings set`, `dconf write`, `systemctl` hay `sudo`.

Mỗi ca in dòng `RESULT <ca> PASS|FAIL`. Nếu GNOME Shell chết, ca bị ghi là FAIL, không treo. Để giữ thư mục làm việc khi gỡ lỗi, đặt `E2E_KEEP=1`.

## Đổi luật gõ

Luật nằm trong `rustype-core/src/tables.rs` và `rustype-core/src/telex.rs`, sửa trực tiếp. Sau khi sửa, `cargo test` sẽ báo những chuỗi phím có kết quả khác trước. Quy trình:

1. Build lại: `cargo build --release`.
2. Xem mọi trường hợp bị đổi: `python3 -I tools/golden.py diff`.
3. Nếu mọi thay đổi đều đúng ý, ghi nhận chúng: `python3 -I tools/golden.py update`.
4. Ghi thay đổi vào [CHANGELOG.md](../CHANGELOG.md).

Muốn tạo lại toàn bộ chuỗi phím đầu vào, ví dụ khi thêm loại trường hợp mới vào `tools/golden.py`, dùng `python3 -I tools/golden.py update --new`.

## Gỡ lỗi trên máy thật

Biến `RUSTYPE_DEBUG=1` làm `rustype-linux` in mã từng phím ra stderr (chỉ mã phím, không in chữ người dùng gõ). Khi chạy dưới systemd, stderr đi vào journal:

```
systemctl --user edit org.freedesktop.IBus.session.GNOME.service
```

Trong trình soạn thảo mở ra, thêm:

```
[Service]
Environment=RUSTYPE_DEBUG=1
```

Sau đó khởi động lại unit, nhấn Super+Space để chuyển sang nguồn nhập khác rồi chuyển lại Rustype, và xem log:

```
journalctl --user -u org.freedesktop.IBus.session.GNOME.service -f
```

Gỡ lỗi xong, xóa dòng `Environment=` (hoặc chạy `systemctl --user revert org.freedesktop.IBus.session.GNOME.service`), rồi khởi động lại unit.

> Log chứa mã phím, có thể suy ra nội dung gõ. Đừng gửi log cho người khác nếu chưa đọc lại.

## Fuzz

Fuzz lõi Telex bằng `cargo-fuzz` (cần cài riêng: `cargo install cargo-fuzz`):

```
cd fuzz
cargo fuzz run fuzz_telex -- -max_total_time=300
```

Fuzz chạy không giới hạn nếu không đặt `max_total_time`. Crash được lưu vào `fuzz/artifacts/`.

## Đóng gói và phát hành

1. **Đóng gói .deb cục bộ**:
   ```bash
   make deb
   ```
   Tạo file `.deb` tại `target/packages/rustype_*.deb`.

2. **Quy trình phát hành bản mới**:
   - Kiểm tra mã nguồn: `make check`.
   - Cập nhật `version` trong `Cargo.toml` và `packaging/debian/changelog`.
   - Ghi chú thay đổi vào [CHANGELOG.md](../CHANGELOG.md).
   - Đẩy tag lên GitHub (ví dụ: `git tag v0.1.0 && git push origin v0.1.0`). GitHub Actions sẽ biên dịch, sinh file `.deb`, tarball, tính checksum SHA256 và đăng tải bản phát hành.

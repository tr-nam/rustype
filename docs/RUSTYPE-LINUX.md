# rustype-linux: bus IBus tự chứa (thử nghiệm)

Tài liệu này dành cho người phát triển dự án. Nó mô tả `rustype-linux`, một tiến trình duy nhất nói giao thức IBus với các client libibus (GNOME Shell, module GTK và Qt) và chạy engine Rustype ngay trong tiến trình đó.

## 1. Trạng thái

- **Thử nghiệm.** Đã kiểm chứng trong sandbox riêng (`dbus-run-session`), với client libibus thật (`tools/ibus_client.py`, `tools/rustype_linux_panel_test.py` và `tools/rustype_linux_shell_preflight.py`).
- **Đã chạy trên GNOME Shell thật của một máy** (Ubuntu GNOME Wayland), qua `tools/rustype-linux-try.sh`. Chưa thử rộng trên máy hay phiên bản GNOME khác.
- Chưa phải là đường cài đặt mặc định. Bộ gõ đang dùng vẫn là `ibus-daemon` cộng `ibus-engine-rustype`.

## 2. Kiến trúc

Trước:

```
Ứng dụng ──D-Bus──> ibus-daemon ──D-Bus──> ibus-engine-rustype (engine)
```

Sau:

```
Ứng dụng ──D-Bus──> rustype-linux (bus + IBus + engine trong một tiến trình)
```

Bỏ đi một tiến trình và một chặng IPC cho mỗi phím. Engine không đổi: `ime.rs` và `telex.rs` được dùng nguyên.

Mã nằm trong `rustype-linux/src/`:

| File | Vai trò |
|---|---|
| `main.rs` | Dòng lệnh, ghi file địa chỉ, dọn dẹp khi thoát, `--own-session-name` |
| `server.rs` | Vòng lặp `poll` một luồng, đọc/ghi không chặn, lùi áp lực đầu vào |
| `sasl.rs` | Bắt tay SASL EXTERNAL phía server |
| `wire.rs` | Tách khung message theo header, mã hóa lại có sender và destination |
| `bus.rs` | Bus: tên unique và well-known, match rule, định tuyến, RequestName |
| `service.rs` | Object `org.freedesktop.IBus`, input context, client panel, `org.freedesktop.IBus.Rustype` |
| `addrfile.rs` | File địa chỉ kiểu ibus-daemon, và việc từ chối khi file trỏ tới tiến trình đang sống |
| `sys.rs` | `poll`, `SO_PEERCRED`, `kill`, bắt SIGINT/SIGTERM (các hàm libc tối thiểu) |

## 3. Hỗ trợ và chưa hỗ trợ

**Có:**
- Bắt tay SASL EXTERNAL, bus có tên unique, RequestName, ReleaseName, NameHasOwner, ListNames, AddMatch, GetId.
- Object IBus: CreateInputContext, ListEngines, SetGlobalEngine, GetGlobalEngine, Properties.
- Input context: ProcessKeyEvent, FocusIn/Out, Reset, SetSurroundingText, ContentType, ClientCommitPreedit, PropertyActivate.
- Panel: gọi FocusIn/FocusOut, RegisterProperties, UpdateProperty, ContentType, DestroyContext trên `org.freedesktop.IBus.Panel`. Nhận PropertyActivate từ panel để bật tắt tiếng Việt, đổi Telex/VNI, mở cài đặt.
- Engine: `rustype` và `xkb:us::eng` (xkb là passthrough).

**Chưa hỗ trợ:**
- `Factory.CreateEngine`: engine IBus khác không chạy được. Chỉ có `rustype` và `xkb:us::eng`.
- ibus-portal: ứng dụng Flatpak chưa có bộ gõ.
- Emoji panel.
- XIM.
- `EffectivePostProcessKeyEvent`: nhận rồi bỏ qua. `ProcessKeyEvent` trả lời trực tiếp, không xếp hàng hậu xử lý.
- Abstract socket: chỉ hỗ trợ `unix:path=`.
- input-method-v2 cho wlroots và KWin: không liên quan tới bus này, vẫn là việc riêng.
- Big-endian: message được chuyển tiếp nguyên bytes, không thêm sender.
- Kết nối session bus (`--own-session-name`) chỉ được giữ để giữ tên, không phục vụ message.
- `--replace` ghi đè file địa chỉ nhưng không dừng tiến trình cũ.

## 4. Lỗi thanh URL của Chrome

Lỗi ký tự lặp ở thanh URL của Chrome được sửa trong `rustype-ibus/src/ime.rs`: ô có mục đích URL vẫn dùng Surrounding Text không gạch chân, để gợi ý và phím Enter hoạt động như gốc. Chrome bỏ qua `DeleteSurroundingText` khi đang chọn gợi ý, nhưng vẫn áp dụng commit. Vì vậy chỉ ở ô URL, engine yêu cầu Surrounding Text, nhớ phần đuôi có thể có, và khi thấy lệnh xóa bị bỏ qua thì xóa đuôi đó rồi commit lại từ. Preedit không được dùng vì làm mất gợi ý khi nhấn Enter, và phím chuyển tiếp không hoạt động trong Chrome trên GNOME Wayland. Sửa này áp dụng cho engine IBus thông thường và cũng có trong `rustype-linux`, vì cả hai dùng chung `ime.rs`. Bản thân `rustype-linux` không phải là thứ sửa lỗi này.

## 5. Số đo

Đo bản release trên cùng một máy, 5 lần mỗi bên, n=243 mỗi lần, tính vòng phím qua socket:

| | Trung vị | p95 |
|---|---|---|
| rustype-linux | 189–228 µs | 245–388 µs |
| ibus-daemon + ibus-engine-rustype | 435–502 µs | 575–622 µs |

Con số này **không bao gồm** Mutter, JavaScript của GNOME Shell và ứng dụng. Lợi ích đầu-cuối thực tế chưa được đo và sẽ nhỏ hơn chênh lệch trên.

## 6. Cách chạy test

```
cargo test -p rustype-linux
tools/rustype_linux_sandbox.sh target/release/rustype-linux
```

Script chạy trong `dbus-run-session` riêng, với `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR` và `IBUS_ADDRESS_FILE` tạm. Nó không đụng vào `~/.config/ibus` hay session bus thật. Sau `tools/ibus_client.py` và panel test, script kiểm tra file địa chỉ và socket đã được xóa khi nhận SIGTERM.

## 7. Tiếp quản trên GNOME (thử nghiệm)

Mới thử trên một máy. Hãy dùng `tools/rustype-linux-try.sh enable`: script hẹn giờ tự khôi phục ibus sau 2 phút, trừ khi chạy `keep`. Các bước thủ công bên dưới chỉ để tham khảo.

Tạo file `~/.config/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/override.conf`:

```
[Service]
ExecStart=
ExecStart=/đường/dẫn/tới/rustype-linux --own-session-name
```

Dòng `ExecStart=` trống xóa lệnh cũ, dòng sau thay bằng `rustype-linux`. Sau đó chạy `systemctl --user daemon-reload` và khởi động lại unit.

Không cần `--replace`, vì systemd dừng daemon cũ trước khi khởi động tiến trình mới.

**Cách gỡ:** xóa file override, chạy `systemctl --user daemon-reload`, rồi khởi động lại `org.freedesktop.IBus.session.GNOME.service`.

**Rủi ro:** nếu tiến trình mới lỗi, không có bộ gõ nào hoạt động cho tới khi gỡ override. Hãy lưu sẵn đường lui trước khi thử, ví dụ mở sẵn terminal để chạy lệnh gỡ.

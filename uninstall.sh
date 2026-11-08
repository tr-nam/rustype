#!/usr/bin/env bash
# Gỡ rustype-linux và các thành phần của Rustype cho người dùng hiện tại.
# Giữ lại ~/.config/rustype để lần sau cài lại không mất cấu hình. Không cần sudo, trừ khi còn
# file component XML của bản cũ trong /usr/share/ibus/component.
#
# Biến SYSTEMCTL chỉ dùng để thử với stub.
set -euo pipefail

SYSTEMCTL=${SYSTEMCTL:-systemctl}
UNIT=org.freedesktop.IBus.session.GNOME.service
TIMER=rustype-rollback.timer
CONF_HOME=${XDG_CONFIG_HOME:-$HOME/.config}
DROPIN_DIR="$CONF_HOME/systemd/user/$UNIT.d"
DROPIN="$DROPIN_DIR/override.conf"

main_pid() { $SYSTEMCTL --user show -p MainPID --value "$UNIT" 2>/dev/null || echo 0; }
exe_of() { readlink "/proc/$1/exe" 2>/dev/null || true; }

# 1. Hủy hẹn giờ rollback, nếu còn, để nó không khởi động lại IBus sau khi gỡ.
$SYSTEMCTL --user stop "$TIMER" >/dev/null 2>&1 || true

# 2. Gỡ nguồn nhập Rustype khỏi GNOME.
if command -v gsettings >/dev/null 2>&1; then
    sources="$(gsettings get org.gnome.desktop.input-sources sources)"
    if [[ "$sources" == *"('ibus', 'rustype')"* ]]; then
        new="$(python3 -I -c '
import ast, sys
items = [t for t in ast.literal_eval(sys.argv[1]) if t != ("ibus", "rustype")]
print("[" + ", ".join("(%r, %r)" % t for t in items) + "]" if items else "@a(ss) []")
' "$sources")"
        gsettings set org.gnome.desktop.input-sources sources "$new"
    fi
fi

# 3. Gỡ drop-in và khởi động lại unit để ibus-daemon quay lại. Không dùng 'ibus restart'.
had_dropin=0
if [ -f "$DROPIN" ]; then
    rm -f "$DROPIN"
    had_dropin=1
fi
rmdir "$DROPIN_DIR" 2>/dev/null || true
if $SYSTEMCTL --user cat "$UNIT" >/dev/null 2>&1; then
    $SYSTEMCTL --user daemon-reload
    pid=$(main_pid)
    if [ "$had_dropin" = 1 ] || [ "$(basename "$(exe_of "$pid")" 2>/dev/null)" != ibus-daemon ]; then
        $SYSTEMCTL --user restart "$UNIT"
        tries=0
        while :; do
            pid=$(main_pid)
            case "$(exe_of "$pid")" in
                */ibus-daemon) echo "ibus-daemon đang chạy (PID $pid)."; break ;;
            esac
            tries=$((tries + 1))
            if [ "$tries" -ge 10 ]; then
                echo "CẢNH BÁO: MainPID ($pid) chưa phải ibus-daemon. Kiểm tra: $SYSTEMCTL --user status $UNIT"
                break
            fi
            sleep 0.5
        done
    fi
fi

# 4. Xóa binary, liên kết, ứng dụng Cài đặt, extension, biểu tượng.
rm -f "$HOME/.local/bin/rustype-linux" "$HOME/.local/bin/ibus-engine-rustype" "$HOME/.local/bin/rustype-settings"
rm -rf "$HOME/.local/lib/rustype"
rm -rf "$HOME/.local/share/gnome-shell/extensions/rustype-focus@namtr.dev"
rm -f "$HOME/.local/share/applications/dev.namtr.RustypeSettings.desktop" "$HOME/.local/share/applications/rustype-settings.desktop"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/rustype.svg" "$HOME/.local/share/icons/hicolor/scalable/apps/dev.namtr.RustypeSettings.svg"
for s in 16 32 48 64 128 256 512; do
    rm -f "$HOME/.local/share/icons/hicolor/${s}x${s}/apps/rustype.png" "$HOME/.local/share/icons/hicolor/${s}x${s}/apps/dev.namtr.RustypeSettings.png"
done
gtk-update-icon-cache -q -t -f "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

# 5. File component XML của bản cũ (chỉ khi còn tồn tại) cần sudo để xóa.
if [[ -e /usr/share/ibus/component/rustype.xml ]]; then
    if ! sudo rm -f /usr/share/ibus/component/rustype.xml; then
        echo "CẢNH BÁO: chưa xóa được /usr/share/ibus/component/rustype.xml. Chạy: sudo rm -f /usr/share/ibus/component/rustype.xml"
    fi
fi

echo "Đã gỡ Rustype. Cấu hình vẫn ở ~/.config/rustype (xóa tay nếu muốn)."
if [ -f /usr/lib/systemd/user/$UNIT.d/rustype.conf ] || [ -f /usr/local/lib/systemd/user/$UNIT.d/rustype.conf ]; then
    echo "LƯU Ý: một gói hệ thống vẫn cung cấp drop-in cho rustype-linux. Gỡ gói đó để tắt hẳn."
fi

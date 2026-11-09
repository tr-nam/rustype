#!/usr/bin/env bash
# Cài rustype-linux làm bộ gõ IBus của phiên GNOME cho người dùng hiện tại, không cần sudo.
#   build -> ~/.local/lib/rustype/rustype-linux -> drop-in systemd của unit IBus -> khởi động lại
#   -> kiểm tra. Hẹn giờ rollback 120 giây TRƯỚC khi khởi động lại. Kiểm tra đạt thì tự hủy hẹn giờ;
#   không đạt thì hoàn tác ngay và thoát với mã lỗi.
# Chạy lại sau mỗi lần sửa code: drop-in đã có thì chỉ cập nhật binary, khởi động lại và kiểm tra.
#
#   install.sh [--yes]   --yes bỏ qua cảnh báo (bắt buộc khi không có terminal).
#
# Biến SYSTEMCTL, SYSTEMD_RUN, RUSTYPE_HEALTH và CARGO chỉ dùng để thử với stub.
set -euo pipefail
cd "$(dirname "$0")"

say() { printf '%s\n' "$*"; }
die() { printf 'LỖI: %s\n' "$*" >&2; exit 1; }
# Chuỗi được nhúng vào lệnh rollback: đặt trong dấu nháy đơn.
q() { local s=$1; printf "'%s'" "${s//\'/\'\\\'\'}"; }

SYSTEMCTL=${SYSTEMCTL:-systemctl}
SYSTEMD_RUN=${SYSTEMD_RUN:-systemd-run}
CARGO=${CARGO:-$(command -v cargo || echo "$HOME/.cargo/bin/cargo")}
UNIT=org.freedesktop.IBus.session.GNOME.service
TIMER=rustype-rollback.timer
ROLLBACK_UNIT=rustype-rollback
ROLLBACK_SECONDS=120
HEALTH_SECONDS=15
CONF_HOME=${XDG_CONFIG_HOME:-$HOME/.config}
DEST="$HOME/.local/lib/rustype"
BIN="$DEST/rustype-linux"
DROPIN_DIR="$CONF_HOME/systemd/user/$UNIT.d"
DROPIN="$DROPIN_DIR/override.conf"

YES=0
for a in "$@"; do
    case "$a" in
        --yes) YES=1 ;;
        -h|--help) say "Cách dùng: install.sh [--yes]"; exit 0 ;;
        *) die "tham số không hợp lệ: $a" ;;
    esac
done

HEALTH_PY=$(cat <<'PY'
import sys, time
import gi
gi.require_version("IBus", "1.0")
from gi.repository import IBus, GLib
IBus.init()
bus = IBus.Bus()
ctx = GLib.MainContext.default()
end = time.time() + 6
while not bus.is_connected() and time.time() < end:
    while ctx.iteration(False):
        pass
    time.sleep(0.05)
if not bus.is_connected():
    sys.exit(1)
reply = bus.get_connection().call_sync(
    "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner",
    GLib.Variant("(s)", ("org.freedesktop.IBus.Panel",)), GLib.VariantType("(b)"), 0, 3000, None)
if not reply.unpack()[0]:
    sys.exit(1)
sys.exit(0 if bus.get_global_engine() is not None else 1)
PY
)

ensure_gnome() {
    if [[ "${XDG_CURRENT_DESKTOP:-}" != *[Gg][Nn][Oo][Mm][Ee]* ]]; then
        die "không phải phiên GNOME (XDG_CURRENT_DESKTOP=${XDG_CURRENT_DESKTOP:-trống}). rustype-linux chỉ hỗ trợ GNOME. Từ chối."
    fi
}

unit_exists() { $SYSTEMCTL --user cat "$UNIT" >/dev/null 2>&1; }
main_pid() { $SYSTEMCTL --user show -p MainPID --value "$UNIT" 2>/dev/null || echo 0; }
exe_of() { readlink "/proc/$1/exe" 2>/dev/null || true; }

addr_file() {
    local machine display
    machine=$(cat /var/lib/dbus/machine-id 2>/dev/null || cat /etc/machine-id 2>/dev/null || echo machine-id)
    machine=$(printf '%s' "$machine" | tr -d '\n')
    display=${WAYLAND_DISPLAY:-${DISPLAY:-}}
    printf '%s' "${IBUS_ADDRESS_FILE:-$CONF_HOME/ibus/bus/$machine-unix-$display}"
}

# Chỉ một lần cảnh báo. Không có terminal và không có --yes thì từ chối, không chờ đợi.
confirm() {
    [ "$YES" = 1 ] && return 0
    [ -t 0 ] || die "không có terminal để xác nhận. Chạy lại với --yes nếu bạn đã sẵn sàng."
    say "CẢNH BÁO: rustype-linux sẽ thay bộ gõ IBus của phiên GNOME này."
    say "Nếu GNOME Shell lỗi, cả phiên có thể đăng xuất. LƯU CÔNG VIỆC TRƯỚC."
    printf 'Nhấn Enter để tiếp tục, Ctrl+C để hủy: '
    read -r _ || die "không đọc được xác nhận"
}

# Lệnh hoàn tác, dùng chung cho hẹn giờ và rollback ngay: gỡ drop-in, rmdir nếu rỗng, nạp lại, khởi động lại.
rollback_script() {
    printf 'rm -f %s; rmdir %s 2>/dev/null; %s --user daemon-reload; %s --user restart %s' \
        "$(q "$DROPIN")" "$(q "$DROPIN_DIR")" "$(q "$SYSTEMCTL")" "$(q "$SYSTEMCTL")" "$UNIT"
}

# Hẹn giờ rollback. Phải chạy trước khi khởi động lại; nếu bước sau treo, đường lui vẫn chạy.
arm_rollback() {
    $SYSTEMCTL --user stop "$TIMER" >/dev/null 2>&1 || true
    $SYSTEMD_RUN --user --unit="$ROLLBACK_UNIT" --on-active="$ROLLBACK_SECONDS" --collect \
        /bin/sh -c "$(rollback_script)" >/dev/null
}

stop_timer() { $SYSTEMCTL --user stop "$TIMER" >/dev/null 2>&1 || true; }
timer_active() { $SYSTEMCTL --user is-active --quiet "$TIMER"; }

write_dropin() {
    mkdir -p "$DROPIN_DIR"
    local tmp="$DROPIN_DIR/.override.conf.$$"
    printf '[Service]\nExecStart=\nExecStart=%s --own-session-name\nRestart=on-failure\nRestartSec=1\n' "$BIN" > "$tmp"
    mv -f "$tmp" "$DROPIN"
}

# Một lần kiểm tra: unit chạy, MainPID là binary của ta, file địa chỉ ghi đúng MainPID,
# và một client libibus thấy Panel (Shell đã kết nối lại) và có engine toàn cục.
health_once() {
    local pid
    $SYSTEMCTL --user is-active --quiet "$UNIT" || return 1
    pid=$(main_pid)
    case "$pid" in ''|*[!0-9]*|0) return 1 ;; esac
    [ "$(exe_of "$pid")" = "$BIN_REAL" ] || return 1
    grep -q "^IBUS_DAEMON_PID=$pid\$" "$(addr_file)" 2>/dev/null || return 1
    if [ -n "${RUSTYPE_HEALTH:-}" ]; then
        sh -c "$RUSTYPE_HEALTH" >/dev/null 2>&1
    else
        timeout 10 env -u IBUS_ADDRESS python3 -I -c "$HEALTH_PY" >/dev/null 2>&1
    fi
}

health() {
    local deadline=$(( $(date +%s) + HEALTH_SECONDS ))
    while :; do
        if health_once; then return 0; fi
        [ "$(date +%s)" -lt "$deadline" ] || return 1
        sleep 1
    done
}

# Hoàn tác ngay: gỡ drop-in, khởi động lại IBus gốc, hủy hẹn giờ (để không khởi động lại lần nữa).
rollback_now() {
    stop_timer
    rm -f "$DROPIN"
    rmdir "$DROPIN_DIR" 2>/dev/null || true
    $SYSTEMCTL --user daemon-reload || true
    $SYSTEMCTL --user restart "$UNIT" || true
    local tries=0 pid exe
    while :; do
        pid=$(main_pid)
        exe=$(exe_of "$pid")
        case "$exe" in
            */ibus-daemon) say "Đã hoàn tác: ibus-daemon đang chạy (PID $pid)."; return 0 ;;
        esac
        tries=$((tries + 1))
        [ "$tries" -lt 10 ] || break
        sleep 0.5
    done
    say "CẢNH BÁO: sau khi hoàn tác, MainPID ($pid) không phải ibus-daemon. Kiểm tra: $SYSTEMCTL --user status $UNIT"
    return 1
}

# ---- Luồng chính ------------------------------------------------------------

ensure_gnome
unit_exists || die "không tìm thấy unit $UNIT. Kiểm tra bằng: systemctl --user cat $UNIT"
[[ "$BIN" =~ ^[A-Za-z0-9/._+@-]+$ ]] || die "đường dẫn $BIN có ký tự systemd không đọc đúng trong ExecStart."
confirm

say "==> Build rustype-linux (release)"
"$CARGO" build --release --locked -p rustype-linux --quiet || die "build lỗi"
SRC="${CARGO_TARGET_DIR:-target}/release/rustype-linux"
[ -x "$SRC" ] || die "không thấy $SRC sau khi build"

say "==> Chép rustype-linux vào $BIN"
mkdir -p "$DEST"
tmp_bin="$DEST/.rustype-linux.new.$$"
install -m755 "$SRC" "$tmp_bin"
mv -f "$tmp_bin" "$BIN"
"$BIN" --write-config
BIN_REAL=$(readlink -f "$BIN")

had_dropin=0
[ -e "$DROPIN" ] && had_dropin=1
say "==> Ghi drop-in systemd: $DROPIN"
write_dropin

say "==> Hẹn giờ rollback sau $ROLLBACK_SECONDS giây, trước khi khởi động lại IBus"
if ! arm_rollback; then
    if [ "$had_dropin" = 0 ]; then
        rm -f "$DROPIN"
        rmdir "$DROPIN_DIR" 2>/dev/null || true
    fi
    die "không hẹn được giờ rollback, chưa khởi động lại gì."
fi

say "==> Khởi động lại IBus ($UNIT) và kiểm tra trong ${HEALTH_SECONDS} giây"
if $SYSTEMCTL --user daemon-reload && $SYSTEMCTL --user restart "$UNIT" && health; then
    stop_timer
    if timer_active; then
        say "CẢNH BÁO: rustype-linux chạy được nhưng chưa hủy được hẹn giờ rollback ($TIMER)."
        say "Nếu không muốn bị hoàn tác sau $ROLLBACK_SECONDS giây, chạy: $SYSTEMCTL --user stop $TIMER"
        exit 1
    fi
    say "rustype-linux đang chạy trên phiên này. Hẹn giờ rollback đã được hủy tự động."
    # Chỉ dọn engine cũ khi bản mới đã chạy: nếu phải hoàn tác, ibus vẫn còn engine để gõ.
    rm -f "$DEST/ibus-engine-rustype" "$HOME/.local/bin/ibus-engine-rustype"
else
    say "Kiểm tra thất bại: hoàn tác ngay."
    rollback_now || true
    exit 1
fi

say "==> Thêm Rustype vào nguồn nhập của GNOME"
sources="$(gsettings get org.gnome.desktop.input-sources sources)"
if [[ "$sources" != *"('ibus', 'rustype')"* ]]; then
    new="$(python3 -I -c '
import ast, sys
s = sys.argv[1].replace("@a(ss) ", "")
items = ast.literal_eval(s) if s.strip() != "[]" else []
items.insert(1 if items else 0, ("ibus", "rustype"))
print("[" + ", ".join("(%r, %r)" % t for t in items) + "]")
' "$sources")"
    gsettings set org.gnome.desktop.input-sources sources "$new"
fi

EXT_SRC="gnome-shell-extension/rustype-focus@namtr.dev"
EXT_DEST="$HOME/.local/share/gnome-shell/extensions/rustype-focus@namtr.dev"
if [[ -d "$EXT_SRC" ]] && command -v gnome-shell >/dev/null 2>&1; then
    say "==> Cài đặt GNOME Shell extension (báo focus cho app_modes)"
    mkdir -p "$EXT_DEST"
    cp -f "$EXT_SRC"/* "$EXT_DEST/"
    if command -v gnome-extensions >/dev/null 2>&1; then
        gnome-extensions enable "rustype-focus@namtr.dev" >/dev/null 2>&1 || true
    fi
fi

say "==> Cài đặt biểu tượng (icons)"
ICON_BASE="$HOME/.local/share/icons/hicolor"
mkdir -p "$ICON_BASE/scalable/apps"
install -m644 assets/icons/scalable/rustype.svg "$ICON_BASE/scalable/apps/rustype.svg"
ln -sf rustype.svg "$ICON_BASE/scalable/apps/dev.namtr.RustypeSettings.svg"
for s in 16 32 48 64 128 256 512; do
    mkdir -p "$ICON_BASE/${s}x${s}/apps"
    install -m644 "assets/icons/${s}x${s}/rustype.png" "$ICON_BASE/${s}x${s}/apps/rustype.png"
    ln -sf rustype.png "$ICON_BASE/${s}x${s}/apps/dev.namtr.RustypeSettings.png"
done
gtk-update-icon-cache -q -t -f "$ICON_BASE" 2>/dev/null || true

say "==> Cài đặt ứng dụng Cài đặt (rustype-settings)"
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
install -m755 tools/rustype-settings "$HOME/.local/bin/rustype-settings"
# Xóa file desktop cũ để tránh hiển thị 2 launcher
rm -f "$HOME/.local/share/applications/rustype-settings.desktop"
install -m644 tools/dev.namtr.RustypeSettings.desktop "$HOME/.local/share/applications/dev.namtr.RustypeSettings.desktop"
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
ln -sf "$BIN" "$HOME/.local/bin/rustype-linux"

if [ -e /usr/share/ibus/component/rustype.xml ]; then
    say "LƯU Ý: còn file /usr/share/ibus/component/rustype.xml của bản cũ. Chạy ./uninstall.sh để xóa (cần sudo)."
fi

say ""
say "==> rustype-linux --doctor"
timeout 10 "$BIN" --doctor || say "(rustype-linux --doctor thoát với mã lỗi $?)"

say ""
say "Xong. Nhấn Super+Space để chuyển sang Rustype."
say "Cấu hình: ~/.config/rustype/config"
say "Gỡ cài đặt: ./uninstall.sh"

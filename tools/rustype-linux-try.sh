#!/usr/bin/env bash
# Thử rustype-linux trên phiên GNOME thật, có đường lui tự động.
#
#   enable [--yes]   build, cài vào ~/.local/lib/rustype, ghi drop-in, khởi động lại IBus,
#                    kiểm tra; hẹn giờ rollback 120 giây trước khi khởi động lại.
#   keep             hủy hẹn giờ rollback, giữ rustype-linux.
#   disable          gỡ drop-in, khởi động lại ibus-daemon, kiểm tra. Chạy được cả khi rustype-linux treo.
#   status           daemon nào đang chạy, còn bao nhiêu giây trước rollback, RAM đang dùng.
#
# Biến môi trường SYSTEMCTL, SYSTEMD_RUN và RUSTYPE_TRY_HEALTH chỉ dùng để thử với stub.
set -eu

SELF=$(readlink -f "$0")
REPO=$(dirname "$(dirname "$SELF")")
SYSTEMCTL=${SYSTEMCTL:-systemctl}
SYSTEMD_RUN=${SYSTEMD_RUN:-systemd-run}
export SYSTEMCTL SYSTEMD_RUN
UNIT=org.freedesktop.IBus.session.GNOME.service
TIMER=rustype-linux-rollback.timer
ROLLBACK_UNIT=rustype-linux-rollback
ROLLBACK_SECONDS=120
CONF_HOME=${XDG_CONFIG_HOME:-$HOME/.config}
DROPIN_DIR="$CONF_HOME/systemd/user/$UNIT.d"
DROPIN="$DROPIN_DIR/override.conf"
INSTALL_BIN="$HOME/.local/lib/rustype/rustype-linux"

say() { printf '%s\n' "$*"; }
die() { printf 'LỖI: %s\n' "$*" >&2; exit 1; }

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

main_pid() { $SYSTEMCTL --user show -p MainPID --value "$UNIT" 2>/dev/null || echo 0; }
exe_of() { readlink "/proc/$1/exe" 2>/dev/null || true; }

addr_file() {
    local machine display
    machine=$(cat /var/lib/dbus/machine-id 2>/dev/null || cat /etc/machine-id 2>/dev/null || echo machine-id)
    machine=$(printf '%s' "$machine" | tr -d '\n')
    display=${WAYLAND_DISPLAY:-${DISPLAY:-}}
    printf '%s' "${IBUS_ADDRESS_FILE:-$CONF_HOME/ibus/bus/$machine-unix-$display}"
}

ensure_gnome() {
    if ! printf '%s' "${XDG_CURRENT_DESKTOP:-}" | grep -qi gnome; then
        die "không phải phiên GNOME (XDG_CURRENT_DESKTOP=${XDG_CURRENT_DESKTOP:-trống}). Từ chối."
    fi
}

unit_exists() { $SYSTEMCTL --user cat "$UNIT" >/dev/null 2>&1; }

# Một lần kiểm tra: unit chạy, MainPID là binary của ta, file địa chỉ trỏ tới MainPID,
# và một client libibus thấy Panel (Shell đã kết nối lại) và đọc được engine hiện tại.
health_once() {
    local pid
    $SYSTEMCTL --user is-active --quiet "$UNIT" || return 1
    pid=$(main_pid)
    case "$pid" in ''|*[!0-9]*|0) return 1 ;; esac
    [ "$(exe_of "$pid")" = "$INSTALL_BIN" ] || return 1
    grep -q "^IBUS_DAEMON_PID=$pid\$" "$(addr_file)" 2>/dev/null || return 1
    if [ -n "${RUSTYPE_TRY_HEALTH:-}" ]; then
        sh -c "$RUSTYPE_TRY_HEALTH" >/dev/null 2>&1
    else
        timeout 10 env -u IBUS_ADDRESS python3 -I -c "$HEALTH_PY" >/dev/null 2>&1
    fi
}

health() {
    local deadline
    deadline=$(( $(date +%s) + 15 ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
        if health_once; then return 0; fi
        sleep 1
    done
    return 1
}

# Gỡ drop-in, khởi động lại ibus-daemon. Dùng cho cả rollback thủ công, rollback đã hẹn giờ và
# rollback ngay sau một lần bật thất bại. $1 = 1 khi do bộ hẹn giờ gọi (không tự dừng bộ hẹn giờ).
disable_core() {
    local from_timer=$1 had=0 is_ibus=0 pid
    if [ "$from_timer" != 1 ]; then
        $SYSTEMCTL --user stop "$TIMER" >/dev/null 2>&1 || true
    fi
    if [ -f "$DROPIN" ]; then
        had=1
        rm -f "$DROPIN"
    fi
    rmdir "$DROPIN_DIR" 2>/dev/null || true
    $SYSTEMCTL --user daemon-reload
    pid=$(main_pid)
    case "$(exe_of "$pid")" in */ibus-daemon) is_ibus=1 ;; esac
    if [ "$had" = 1 ] || [ "$is_ibus" = 0 ]; then
        $SYSTEMCTL --user restart "$UNIT"
    fi
    # systemd may report the new MainPID a moment after restart returns: poll briefly.
    local tries=0
    while :; do
        pid=$(main_pid)
        case "$(exe_of "$pid")" in
            */ibus-daemon) say "Đã gỡ. ibus-daemon đang chạy (PID $pid)."; return 0 ;;
        esac
        tries=$((tries + 1))
        [ "$tries" -lt 10 ] || break
        sleep 0.5
    done
    say "CẢNH BÁO: sau khi gỡ, MainPID ($pid) không phải ibus-daemon. Kiểm tra bằng 'status'."
    return 1
}

cmd_enable() {
    local yes=0 src target
    for a in "$@"; do
        case "$a" in
            --yes) yes=1 ;;
            *) die "tham số không hợp lệ: $a" ;;
        esac
    done
    ensure_gnome
    unit_exists || die "không tìm thấy unit $UNIT. Kiểm tra bằng: systemctl --user cat $UNIT"
    [ ! -e "$DROPIN" ] || die "đã có $DROPIN, rustype-linux đang được bật. Chạy 'disable' trước."

    if [ "$yes" != 1 ]; then
        say "CẢNH BÁO: nếu GNOME Shell lỗi, cả phiên có thể đăng xuất. Lưu công việc trước."
        printf 'Nhấn Enter để tiếp tục, Ctrl+C để hủy: '
        read -r _ </dev/tty || die "không đọc được xác nhận"
    fi

    say "Đang build rustype-linux (release)..."
    (cd "$REPO" && cargo build --release -p rustype-linux --quiet) || die "build lỗi"
    target=${CARGO_TARGET_DIR:-$REPO/target}
    src="$target/release/rustype-linux"
    [ -x "$src" ] || die "không thấy $src sau khi build"

    mkdir -p "$(dirname "$INSTALL_BIN")"
    local tmp_bin="$(dirname "$INSTALL_BIN")/.rustype-linux.new.$$"
    cp "$src" "$tmp_bin"
    chmod 755 "$tmp_bin"
    mv -f "$tmp_bin" "$INSTALL_BIN"

    mkdir -p "$DROPIN_DIR"
    local tmp_conf="$DROPIN_DIR/.override.conf.$$"
    printf '[Service]\nExecStart=\nExecStart=%s --own-session-name\n' "$INSTALL_BIN" > "$tmp_conf"
    mv -f "$tmp_conf" "$DROPIN"

    # Hẹn giờ rollback TRƯỚC khi khởi động lại: nếu bước sau treo, đường lui vẫn chạy.
    if ! $SYSTEMD_RUN --user --unit="$ROLLBACK_UNIT" --on-active="$ROLLBACK_SECONDS" --collect \
        "$SELF" disable --from-timer >/dev/null; then
        rm -f "$DROPIN"
        rmdir "$DROPIN_DIR" 2>/dev/null || true
        die "không hẹn được giờ rollback, chưa khởi động lại gì. Đã gỡ drop-in."
    fi

    if $SYSTEMCTL --user daemon-reload && $SYSTEMCTL --user restart "$UNIT" && health; then
        say ""
        say "rustype-linux đang chạy trên phiên này."
        say "Hãy thử ngay:"
        say "  - Gõ 'Traanf' trong ô văn bản và thanh địa chỉ: phải ra 'Trần'."
        say "  - Bật/tắt tiếng Việt từ menu IBus trên thanh trên."
        say "  - Đổi sang bàn phím tiếng Anh rồi đổi lại."
        say "  - Mở một ô mật khẩu và kiểm tra gõ bình thường."
        say ""
        say "Nếu có gì sai: chạy '$SELF disable'."
        say "Nếu không làm gì, đường lui tự chạy sau $ROLLBACK_SECONDS giây."
        say "Muốn giữ rustype-linux, chạy '$SELF keep' trong vòng 2 phút."
    else
        say "Kiểm tra thất bại: hoàn tác ngay."
        disable_core 0 || true
        exit 1
    fi
}

cmd_keep() {
    [ -f "$DROPIN" ] || die "rustype-linux chưa được bật, không có gì để giữ."
    $SYSTEMCTL --user stop "$TIMER" >/dev/null 2>&1 || true
    if $SYSTEMCTL --user is-active --quiet "$TIMER"; then
        die "chưa hủy được hẹn giờ rollback. Chạy '$SELF disable' nếu muốn gỡ."
    fi
    say "Đã hủy hẹn giờ rollback. rustype-linux được giữ lại."
}

cmd_disable() {
    local from_timer=0
    for a in "$@"; do
        case "$a" in
            --from-timer) from_timer=1 ;;
            *) die "tham số không hợp lệ: $a" ;;
        esac
    done
    disable_core "$from_timer"
}

cmd_status() {
    local pid exe left next now
    pid=$(main_pid)
    exe=$(exe_of "$pid")
    say "MainPID: $pid ${exe:+($exe)}"
    if [ -f "$DROPIN" ]; then say "rustype-linux: đang bật (drop-in $DROPIN)"; else say "rustype-linux: chưa bật"; fi
    if $SYSTEMCTL --user is-active --quiet "$TIMER"; then
        next=$($SYSTEMCTL --user show -p NextElapseUSecRealtime --value "$TIMER" 2>/dev/null || echo 0)
        now=$(date +%s)
        left=$(( next / 1000000 - now ))
        [ "$left" -gt 0 ] 2>/dev/null && say "Rollback tự động sau: $left giây" || say "Rollback: đang chờ chạy"
    else
        say "Không có hẹn giờ rollback."
    fi
    case "$exe" in
        */rustype-linux)
            say "RAM rustype-linux: $(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ') KB" ;;
        */ibus-daemon)
            local rss children
            rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ')
            children=$(ps -o rss= --ppid "$pid" 2>/dev/null | awk '{s+=$1} END {print s+0}')
            say "RAM ibus-daemon: $rss KB, các tiến trình con: ${children} KB" ;;
        *) say "Không có RAM để báo cáo." ;;
    esac
}

usage() {
    say "Cách dùng: $(basename "$SELF") {enable [--yes] | keep | disable | status}"
}

case "${1:-}" in
    enable) shift; cmd_enable "$@" ;;
    keep) cmd_keep ;;
    disable) shift; cmd_disable "$@" ;;
    status) cmd_status ;;
    *) usage; exit 2 ;;
esac

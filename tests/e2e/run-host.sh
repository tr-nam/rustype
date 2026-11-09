#!/usr/bin/env bash
# One end-to-end case on this machine: private buses, private headless GNOME Shell, the engine behind
# an ibus-daemon PATH stub, then tests/e2e/driver.py. Each case is one fresh session.
# usage: run-host.sh ENGINE_BIN CASE [WORD]    (CASE: a b c d e, or D for the address-bar word case)
# Requires: timeout -k 5 150 wraps the call (done by the Makefile target).
set -u
E2E_LIB="$(cd "$(dirname "$0")" && pwd)/lib.sh"; export E2E_LIB
. "$E2E_LIB"
ENGINE=$(readlink -f "$1"); CASE=$2; WORD=${3:-}
[ -x "$ENGINE" ] || { echo "no engine at $ENGINE" >&2; exit 2; }
HERE=$(dirname "$E2E_LIB")
e2e_make_work
export E2E_WORK E2E_SYSTEM_BUS E2E_REAL_SESSION_BUS
LOGS="$E2E_WORK/logs"
echo "$ENGINE" > "$E2E_WORK/engine-path"
cat > "$E2E_WORK/stubs/ibus-daemon" <<STUB
#!/usr/bin/env bash
# PATH stub: GNOME Shell starts "ibus-daemon --panel disable" when no IBus unit exists.
exec env RUSTYPE_DEBUG=1 "$ENGINE" --address unix:path=$E2E_WORK/xdg/ibus.sock --own-session-name --replace 2>>"$LOGS/rustype-linux.log"
STUB
chmod +x "$E2E_WORK/stubs/ibus-daemon"
mkdir -p "$E2E_WORK/config/glib-2.0/settings"
printf '[org/gnome/desktop/input-sources]\nsources=[('"'"'ibus'"'"', '"'"'rustype'"'"')]\n' > "$E2E_WORK/config/glib-2.0/settings/keyfile"
PIDS="$E2E_WORK/logs/pgids"; : > "$PIDS"
cleanup() {
    # Kill only the process groups this run created (recorded in $PIDS).
    while read -r g; do [ -n "$g" ] && kill -TERM -- "-$g" 2>/dev/null; done < "$PIDS"
    sleep 2
    while read -r g; do [ -n "$g" ] && kill -KILL -- "-$g" 2>/dev/null; done < "$PIDS"
    if [ -n "${E2E_KEEP:-}" ]; then echo "kept work dir: $E2E_WORK" >&2; else rm -rf "$E2E_WORK"; fi
}
trap cleanup EXIT

# Private buses. The session bus comes from dbus-run-session; its address is written to a file.
E2E_SYSTEM_BUS="unix:abstract=rustype-e2e-sys-$$"; export E2E_SYSTEM_BUS
setsid env -i HOME="$E2E_WORK/home" XDG_RUNTIME_DIR="$E2E_WORK/xdg" PATH=/usr/bin:/bin \
    dbus-daemon --session --address="$E2E_SYSTEM_BUS" --nofork --print-address=1 \
    > "$LOGS/sysbus.out" 2> "$LOGS/sysbus.err" < /dev/null &
echo $! >> "$PIDS"
setsid env -i HOME="$E2E_WORK/home" XDG_RUNTIME_DIR="$E2E_WORK/xdg" PATH=/usr/bin:/bin \
    dbus-run-session -- bash -c 'echo "$DBUS_SESSION_BUS_ADDRESS" > "$1"; sleep 600' _ "$LOGS/session.addr" \
    > "$LOGS/session.out" 2>&1 < /dev/null &
echo $! >> "$PIDS"
for _ in $(seq 100); do [ -s "$LOGS/session.addr" ] && break; sleep 0.1; done
SESSION=$(cat "$LOGS/session.addr")
[ -n "$SESSION" ] || { echo "no private session bus" >&2; exit 3; }
export E2E_SESSION="$SESSION"

# Headless GNOME Shell, its own process group.
setsid "$HERE/sb-wrap.sh" "$SESSION" env GSETTINGS_BACKEND=keyfile gnome-shell --headless --wayland --no-x11 \
    --virtual-monitor 1280x800 --wayland-display=rustype-e2e-0 > "$LOGS/shell.log" 2>&1 < /dev/null &
SHELL_PID=$!; echo "$SHELL_PID" >> "$PIDS"
echo "shell pid $SHELL_PID" >&2

# Wait for the IBus name and check the owner by PID (the stub makes rustype-linux the owner).
ready=0
for _ in $(seq 200); do
    owner=$("$HERE/sb-wrap.sh" "$SESSION" gdbus call --session --dest org.freedesktop.DBus \
        --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.GetNameOwner org.freedesktop.IBus 2>/dev/null \
        | sed -n "s/.*'\(:[^']*\)'.*/\1/p")
    [ -n "$owner" ] && { ready=1; break; }; sleep 0.2
done
[ "$ready" = 1 ] || { echo "RESULT $CASE FAIL ibus name never owned"; cat "$LOGS/shell.log" | tail -n 20 >&2; exit 3; }

# The case itself: driver.py talks to the private session bus and starts the apps.
"$HERE/sb-wrap.sh" "$SESSION" python3 -I "$HERE/driver.py" "$CASE" "$WORD" "$E2E_WORK" "$LOGS" "$SHELL_PID"
RC=$?
echo "driver exit $RC"
echo "--- rustype-linux log (tail)"; tail -n 6 "$LOGS/rustype-linux.log" 2>/dev/null
exit $RC

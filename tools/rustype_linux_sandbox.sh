#!/usr/bin/env bash
# Private dbus + private rustype-linux. The real IBus session and ~/.config/ibus are never touched.
# Runs the unmodified ibus_client.py (same assertions as ibus_sandbox_test.sh), then the panel
# test (a real libibus PanelService owning org.freedesktop.IBus.Panel), then checks the server's
# own cleanup. usage: tools/rustype_linux_sandbox.sh path/to/rustype-linux
set -u
H=$(cd "$(dirname "$0")" && pwd)
BIN=${1:-}
if [ -z "$BIN" ] || [ ! -x "$BIN" ]; then
    echo "usage: $0 path/to/rustype-linux" >&2
    exit 2
fi
BIN=$(readlink -f "$BIN")
T=$(mktemp -d "${TMPDIR:-/tmp}/rustype-linux-test.XXXX")
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/config" "$T/cache" "$T/run"
chmod 700 "$T/run"

dbus-run-session -- env -u IBUS_ADDRESS -u WAYLAND_DISPLAY \
    IBUS_ADDRESS_FILE="$T/addr" XDG_CONFIG_HOME="$T/config" XDG_CACHE_HOME="$T/cache" \
    XDG_RUNTIME_DIR="$T/run" DISPLAY=:97 \
    bash -c '
        T=$1; H=$2; BIN=$3
        "$BIN" --address "unix:path=$T/sock" --own-session-name > "$T/server.log" 2>&1 &
        D=$!
        for _ in $(seq 100); do [ -s "$T/addr" ] && [ -S "$T/sock" ] && break; sleep 0.1; done
        RC=0

        # Claimed on the private session bus (the systemd unit BusName).
        OWNED=$(gdbus call --session --dest org.freedesktop.DBus \
            --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner \
            org.freedesktop.IBus 2>/dev/null)
        case "$OWNED" in
            *true*) echo "OK   session bus name org.freedesktop.IBus owned" ;;
            *) echo "FAIL session bus name org.freedesktop.IBus not owned: $OWNED"; RC=1 ;;
        esac

        echo "--- ibus_client.py (unmodified)"
        IBUS_ADDRESS="unix:path=$T/sock" timeout 120 python3 -I "$H/ibus_client.py" rustype
        RC_CLIENT=$?
        echo "--- rustype_linux_panel_test.py"
        IBUS_ADDRESS="unix:path=$T/sock" timeout 120 python3 -I "$H/rustype_linux_panel_test.py"
        RC_PANEL=$?

        kill -TERM $D
        wait $D
        RC_SERVER=$?
        if [ -e "$T/addr" ] || [ -e "$T/sock" ]; then
            echo "FAIL server left its address file or socket behind"; RC=1
        else
            echo "OK   address file and socket removed on SIGTERM"
        fi
        if [ "$RC_SERVER" -ne 0 ]; then echo "FAIL server exit status $RC_SERVER"; RC=1; fi

        # Stage 3: the GNOME Shell call sequence, then reconnect to a new server on a new socket.
        # Own address file, and G_DEBUG=fatal-criticals: any GLib CRITICAL fails the stage.
        echo "--- rustype_linux_shell_preflight.py"
        IBUS_ADDRESS_FILE="$T/addr2" G_DEBUG=fatal-criticals timeout 300 python3 -I "$H/rustype_linux_shell_preflight.py" "$BIN" "$T"
        RC_PRE=$?

        echo "client exit $RC_CLIENT, panel test exit $RC_PANEL, server exit $RC_SERVER, shell preflight exit $RC_PRE"
        if [ "$RC_CLIENT" -ne 0 ] || [ "$RC_PANEL" -ne 0 ] || [ "$RC_PRE" -ne 0 ]; then RC=1; fi
        echo "--- server log (last lines)"
        tail -n 5 "$T/server.log"
        exit $RC
    ' _ "$T" "$H" "$BIN" 2>&1 | grep -vE 'Gtk-WARNING|dbus-daemon\[|fusermount|g_object_is_floating|super\(\)\.iteration|^\s*$|connection to the bus'
# PIPESTATUS[0] is the sandbox's own status; grep -v must not mask it.
exit "${PIPESTATUS[0]}"

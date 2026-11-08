#!/usr/bin/env bash
# Private dbus + private ibus-daemon. The real IBus session is never touched.
# usage: tools/ibus_sandbox_test.sh ENGINE_EXEC COMPONENT_XML_TEMPLATE ENGINE_NAME
set -u
H=$(cd "$(dirname "$0")" && pwd)
T=$(mktemp -d "${TMPDIR:-/tmp}/rustype-ibus-test.XXXX")
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/comp" "$T/config" "$T/cache"
# Make the path safe for the XML <exec> element, then for the sed replacement (| is the delimiter).
EXEC_XML=$(printf '%s' "$1 --ibus" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g')
EXEC_ESC=$(printf '%s' "$EXEC_XML" | sed -e 's/[\\|&]/\\&/g')
sed "s|<exec>.*</exec>|<exec>$EXEC_ESC</exec>|" "$2" > "$T/comp/engine.xml" || exit 1
dbus-run-session -- env -u IBUS_ADDRESS -u WAYLAND_DISPLAY \
    IBUS_COMPONENT_PATH="$T/comp" IBUS_ADDRESS_FILE="$T/addr" XDG_CONFIG_HOME="$T/config" XDG_CACHE_HOME="$T/cache" DISPLAY=:97 \
    bash -c '
        T=$1; H=$2; E=$3
        ibus-daemon --address="unix:path=$T/sock" --single --panel=disable --config=disable --emoji-extension=disable --cache=none > "$T/daemon.log" 2>&1 &
        D=$!
        sleep 1.5
        IBUS_ADDRESS="unix:path=$T/sock" timeout 90 python3 -I "$H/ibus_client.py" "$E"
        RC=$?
        echo "--- engine processes under the test daemon:"
        for k in $(ps -o pid= --ppid $D); do ps -o pid=,rss=,args= -p $k; done
        kill $D; wait $D 2>/dev/null
        grep -iE "rustype|critical|error" "$T/daemon.log" | grep -v "Failed to unlink" | head -5
        exit $RC
    ' _ "$T" "$H" "$3" 2>&1 | grep -vE 'Gtk-WARNING|dbus-daemon\[|fusermount|g_object_is_floating|super\(\)\.iteration|^\s*$|connection to the bus'
# PIPESTATUS[0] is the exit status of dbus-run-session, i.e. the client's status (grep -v must not mask it).
exit "${PIPESTATUS[0]}"

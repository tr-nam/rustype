# Shared helpers for the end-to-end suite. Sourced by run-host.sh and docker-run.sh.
# Every process of the stack runs through sb_env: env -i with an explicit variable list, private
# HOME and XDG dirs, a private session bus (dbus-run-session) and a private system bus.
# Nothing here calls gsettings/dconf write, systemctl start/stop, sudo, or kills by name.

# The caller's (real) session bus, recorded once. Inside the sandbox it is passed in, not re-read.
if [ -z "${E2E_REAL_SESSION_BUS+set}" ]; then E2E_REAL_SESSION_BUS="${DBUS_SESSION_BUS_ADDRESS:-}"; fi

# Work dir under /tmp: the UNIX socket path limit (108 bytes) rules out longer paths.
e2e_make_work() {
    E2E_WORK=$(mktemp -d /tmp/rustype-e2e.XXXXXX) || exit 2
    export E2E_WORK
    mkdir -p "$E2E_WORK"/{home,xdg,config,data,cache,state,logs,stubs} && chmod 700 "$E2E_WORK/xdg"
}

# Guard: aborts unless HOME and XDG_RUNTIME_DIR are under the work dir and the session bus is not the real one.
# Runs inside the env -i child, so it checks what the stack actually receives.
e2e_guard() {
    case "$(readlink -f "$HOME")" in "$E2E_WORK"/*) ;; *) echo "GUARD: HOME outside work dir" >&2; exit 90;; esac
    case "$(readlink -f "$XDG_RUNTIME_DIR")" in "$E2E_WORK"/*) ;; *) echo "GUARD: XDG_RUNTIME_DIR outside work dir" >&2; exit 90;; esac
    if [ -n "${E2E_REAL_SESSION_BUS:-}" ] && [ "${DBUS_SESSION_BUS_ADDRESS:-}" = "$E2E_REAL_SESSION_BUS" ]; then
        echo "GUARD: session bus is the real one" >&2; exit 90; fi
    case "${DBUS_SYSTEM_BUS_ADDRESS:-}" in ""|*/run/dbus/system_bus_socket) echo "GUARD: system bus not private" >&2; exit 90;; esac
}

# (sb_env is kept for scripts that source this file and want a one-off env; sb-wrap.sh is the normal path.)
sb_env() {
    local session=$1; shift
    env -i \
        HOME="$E2E_WORK/home" XDG_RUNTIME_DIR="$E2E_WORK/xdg" XDG_CONFIG_HOME="$E2E_WORK/config" \
        XDG_DATA_HOME="$E2E_WORK/data" XDG_CACHE_HOME="$E2E_WORK/cache" XDG_STATE_HOME="$E2E_WORK/state" \
        PATH="$E2E_WORK/stubs:/usr/local/bin:/usr/bin:/bin" LANG=C.UTF-8 GSETTINGS_BACKEND=keyfile \
        DBUS_SESSION_BUS_ADDRESS="$session" DBUS_SYSTEM_BUS_ADDRESS="$E2E_SYSTEM_BUS" \
        WAYLAND_DISPLAY="${E2E_DISPLAY:-rustype-e2e-0}" \
        bash -c '. "'"$E2E_LIB"'"; e2e_guard; exec "$@"' _ "$@"
}

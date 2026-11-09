#!/usr/bin/env bash
# sb-wrap.sh SESSION_ADDR CMD...: runs CMD in the explicit private environment (env -i, guard inside).
SESSION=$1; shift
exec env -i \
    HOME="$E2E_WORK/home" XDG_RUNTIME_DIR="$E2E_WORK/xdg" XDG_CONFIG_HOME="$E2E_WORK/config" \
    XDG_DATA_HOME="$E2E_WORK/data" XDG_CACHE_HOME="$E2E_WORK/cache" XDG_STATE_HOME="$E2E_WORK/state" \
    PATH="$E2E_WORK/stubs:/usr/local/bin:/usr/bin:/bin" LANG=C.UTF-8 GSETTINGS_BACKEND=keyfile \
    DBUS_SESSION_BUS_ADDRESS="$SESSION" DBUS_SYSTEM_BUS_ADDRESS="$E2E_SYSTEM_BUS" \
    WAYLAND_DISPLAY="rustype-e2e-0" E2E_WORK="$E2E_WORK" E2E_SYSTEM_BUS="$E2E_SYSTEM_BUS" \
    E2E_REAL_SESSION_BUS="$E2E_REAL_SESSION_BUS" E2E_LIB="$E2E_LIB" \
    bash -c '. "$E2E_LIB"; e2e_guard; exec "$@"' _ "$@"

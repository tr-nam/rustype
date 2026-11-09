#!/usr/bin/env bash
# Shared helpers for the integration suite. Sourced by tests/install/run.sh, tests/deb/run.sh and
# tests/run_all.sh. Not meant to be run directly.
#
# Each test gets a sandbox (sb_new) that is removed when the test ends:
#   - a private HOME and XDG_* directories under a fresh temp dir;
#   - scripts run under `env -i` with an explicit variable list (sb_run), so the host's
#     DBUS_SESSION_BUS_ADDRESS, DISPLAY and WAYLAND_DISPLAY never reach them;
#   - PATH holds only $SB/stubs (scripted fakes that log every call to $SB/calls.log) and
#     $SB/allow (symlinks to harmless tools). /usr/bin is not on PATH;
#   - BASH_ENV installs command_not_found_handle, which logs NOTFOUND. sb_run fails the test on
#     any NOTFOUND or "command not found" message, even when the script hid it with 2>/dev/null.
#
# Tests are functions. A test runs in a subshell, so `fail` (or a failed assert_*) ends only that
# test. Register them with:
#   test_case   "<name>"        <function> [args]   must pass
#   known_fail  "<bug text>"    <function> [args]   documents a product bug: must fail
# A known_fail that passes prints UNEXPECTED-PASS and counts as a failure.

RT_UNIT=org.freedesktop.IBus.session.GNOME.service
RT_TESTS_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
RT_REPO_ROOT=$(cd "$RT_TESTS_DIR/.." && pwd)
RT_PASS=0
RT_FAIL=0
RT_KNOWN=0
RT_UNEXPECTED=0
RT_SKIP=0
RT_RC=0
SB=""
SB_CWD=""
SB_EXTRA=()

# Harmless tools that the scripts may call. Resolved on the host and symlinked into allow/.
RT_ALLOW_TOOLS=(
    bash sh env cat cp mv rm rmdir mkdir ln ls chmod install readlink realpath basename dirname
    date sleep tr cut head tail wc sort uniq grep sed awk find mktemp mkfifo id uname du stat touch
    cmp diff true false test '[' printf seq tee expr which od python3 gzip xz tar dpkg-deb
)

# ---- runner ----------------------------------------------------------------

# Refuses to run unless RUSTYPE_LINUX_BIN names an executable rustype-linux binary.
rt_require_bin() {
    if [ -z "${RUSTYPE_LINUX_BIN:-}" ]; then
        echo "RUSTYPE_LINUX_BIN is not set. Build rustype-linux and point it at the binary (absolute path)." >&2
        exit 2
    fi
    case $RUSTYPE_LINUX_BIN in
        /*) ;;
        *) echo "RUSTYPE_LINUX_BIN must be an absolute path: $RUSTYPE_LINUX_BIN" >&2; exit 2 ;;
    esac
    if [ ! -x "$RUSTYPE_LINUX_BIN" ]; then
        echo "RUSTYPE_LINUX_BIN is not executable: $RUSTYPE_LINUX_BIN" >&2
        exit 2
    fi
}

test_case() { _rt_run pass "$@"; }
known_fail() { _rt_run known "$@"; }

# Call from inside a test to skip it (for example when the host state the test needs is missing).
skip_test() {
    echo "$*"
    exit 77
}

_rt_run() {
    local kind=$1 label=$2 fn=$3 out rc
    shift 3
    # RT_FILTER=<text> runs only the tests whose label or function name contains <text>.
    if [ -n "${RT_FILTER:-}" ] && [[ $label != *"$RT_FILTER"* && $fn != *"$RT_FILTER"* ]]; then
        return 0
    fi
    out=$(mktemp "${TMPDIR:-/tmp}/rt-test.XXXXXX")
    (
        trap '_rt_cleanup' EXIT
        "$fn" "$@"
    ) >"$out" 2>&1
    rc=$?
    case $kind in
        pass)
            if [ "$rc" -eq 0 ]; then
                RT_PASS=$((RT_PASS + 1))
                echo "PASS          $label"
            elif [ "$rc" -eq 77 ]; then
                RT_SKIP=$((RT_SKIP + 1))
                echo "SKIP          $label: $(tail -n1 "$out")"
            else
                RT_FAIL=$((RT_FAIL + 1))
                echo "FAIL          $label"
                sed 's/^/    /' "$out"
            fi
            ;;
        known)
            if [ "$rc" -ne 0 ]; then
                RT_KNOWN=$((RT_KNOWN + 1))
                echo "KNOWN-FAIL    $label"
                sed 's/^/    /' "$out" | tail -n 3
            else
                RT_UNEXPECTED=$((RT_UNEXPECTED + 1))
                echo "UNEXPECTED-PASS $label (the bug looks fixed: drop known_fail for this check)"
            fi
            ;;
    esac
    rm -f "$out"
}

# Prints the summary line. Its exit status is the suite's. When RT_RESULTS_FILE is set (by
# run_all.sh), it also appends "passed failed known-fail" so run_all can total the suites.
rt_summary() {
    local failed=$((RT_FAIL + RT_UNEXPECTED))
    echo
    echo "SUMMARY: passed=$RT_PASS failed=$failed known-fail=$RT_KNOWN skipped=$RT_SKIP"
    if [ -n "${RT_RESULTS_FILE:-}" ]; then
        echo "$RT_PASS $failed $RT_KNOWN" >> "$RT_RESULTS_FILE"
    fi
    [ "$failed" -eq 0 ]
}

_rt_cleanup() {
    local pid
    if [ -n "$SB" ] && [ -d "$SB" ]; then
        if [ -f "$SB/state/pids" ]; then
            while read -r pid; do
                kill "$pid" 2>/dev/null
            done < "$SB/state/pids"
        fi
        case $SB in
            */rustype-it.*) rm -rf -- "$SB" ;;
        esac
    fi
    SB=""
}

# ---- assertions (call inside a test) ---------------------------------------

fail() {
    printf '    assertion failed: %s\n' "$*" >&2
    exit 1
}

assert_rc_zero() {
    [ "$RT_RC" -eq 0 ] || fail "$1: exit $RT_RC, want 0 (stderr tail: $(tail -n3 "$SB/err" | tr '\n' ' '))"
}

assert_rc_nonzero() {
    [ "$RT_RC" -ne 0 ] || fail "$1: exit 0, want a failure"
}

assert_eq() {
    [ "$1" = "$2" ] || fail "$3: got [$1], want [$2]"
}

assert_exists() {
    [ -e "$1" ] || [ -L "$1" ] || fail "missing: $1"
}

assert_absent() {
    if [ -e "$1" ] || [ -L "$1" ]; then
        fail "still present: $1"
    fi
}

assert_mode() {
    local m
    m=$(stat -c %a "$1" 2>/dev/null) || fail "no such file: $1"
    assert_eq "$m" "$2" "mode of $1"
}

assert_same_file() {
    cmp -s "$1" "$2" || fail "$1 differs from $2"
}

# Exact file content, byte for byte (include the trailing newline in EXPECTED).
assert_text() {
    printf '%s' "$2" > "$SB/.expect"
    cmp -s "$1" "$SB/.expect" || fail "content of $1 differs; got [$(cat "$1" 2>/dev/null)]"
}

assert_link() {
    [ -L "$1" ] || fail "not a symlink: $1"
    assert_eq "$(readlink "$1")" "$2" "target of $1"
}

assert_output_has() {
    cat "$SB/out" "$SB/err" | grep -qF -- "$1" || fail "output does not contain: $1"
}

assert_output_lacks() {
    if cat "$SB/out" "$SB/err" | grep -qF -- "$1"; then
        fail "output unexpectedly contains: $1"
    fi
}

# Line number in calls.log of the Nth (or last) line matching the ERE; empty when none.
log_line() {
    if [ "${2:-1}" = last ]; then
        grep -nE -- "$1" "$SB/calls.log" | tail -n1 | cut -d: -f1
    else
        grep -nE -- "$1" "$SB/calls.log" | sed -n "${2:-1}p" | cut -d: -f1
    fi
}

assert_logged() {
    grep -qE -- "$1" "$SB/calls.log" || fail "not logged: $1"
}

assert_not_logged() {
    if grep -qE -- "$1" "$SB/calls.log"; then
        fail "unexpectedly logged: $(grep -E -- "$1" "$SB/calls.log" | head -n1)"
    fi
}

assert_line_logged() {
    grep -qxF -- "$1" "$SB/calls.log" || fail "exact line not logged: $1"
}

assert_logged_count() {
    local n
    n=$(grep -cE -- "$1" "$SB/calls.log")
    assert_eq "$n" "$2" "number of calls matching $1"
}

# First match of A comes before first match of B.
assert_before() {
    local a b
    a=$(log_line "$1" 1)
    b=$(log_line "$2" 1)
    [ -n "$a" ] || fail "not logged: $1"
    [ -n "$b" ] || fail "not logged: $2"
    [ "$a" -lt "$b" ] || fail "order: [$1] (line $a) must come before [$2] (line $b)"
}

# ---- sandbox ---------------------------------------------------------------

# Creates the sandbox, sets SB, and starts a stock ibus-daemon stand-in as the unit's MainPID.
sb_new() {
    local t p n
    SB=$(mktemp -d "${TMPDIR:-/tmp}/rustype-it.XXXXXX") || fail "mktemp failed"
    mkdir -p "$SB/home" "$SB/stubs" "$SB/allow" "$SB/ctl" "$SB/state" "$SB/fake-ibus" \
        "$SB/run/user" "$SB/xdg/config" "$SB/xdg/data" "$SB/xdg/cache" "$SB/xdg/run" \
        || fail "cannot create sandbox dirs"
    chmod 700 "$SB/xdg/run"
    : > "$SB/calls.log"
    : > "$SB/state/pids"
    : > "$SB/passwd"
    : > "$SB/allow-missing"
    SB_CWD=$SB
    SB_EXTRA=()

    for t in "${RT_ALLOW_TOOLS[@]}"; do
        p=$(type -P "$t") || continue
        ln -s "$p" "$SB/allow/$t"
    done
    for n in sudo apt apt-get dpkg systemctl gsettings runuser pkill killall ibus; do
        [ -e "$SB/allow/$n" ] && fail "forbidden tool in the allow list: $n"
    done

    # The ibus stand-in is a copy of rustype-linux named ibus-daemon: the scripts only look at the
    # name of the running executable, and rustype-linux --feed stays alive on an open stdin.
    cp "$RUSTYPE_LINUX_BIN" "$SB/fake-ibus/ibus-daemon" || fail "cannot make the ibus stand-in"
    printf '%s\n' "$RUSTYPE_LINUX_BIN" > "$SB/state/bin-src"
    touch "$SB/state/unit_present"
    printf "[('xkb', 'us')]\n" > "$SB/state/sources"
    _sb_write_stubs
    _sb_start_ibus
}

# Adds an environment variable (KEY=VALUE) to every later sb_run in this test.
sb_env() {
    SB_EXTRA+=("$1")
}

# sb_ctl NAME ERE RC [OUTPUT]: when a call to NAME matches ERE (against "NAME args"), the stub
# prints OUTPUT and exits with RC. The first matching line wins. Without a match the stub exits 0.
sb_ctl() {
    printf '%s\t%s\t%s\n' "$2" "$3" "${4:-}" >> "$SB/ctl/$1"
}

# Removes a stub so the command is missing from the sandbox.
sb_remove_stub() {
    rm -f "$SB/stubs/$1"
}

# Allows a stubbed-out command to be absent without failing the test.
sb_expect_missing() {
    echo "$1" >> "$SB/allow-missing"
}

sb_set_sources() {
    printf '%s\n' "$1" > "$SB/state/sources"
}

# Installs the packaged drop-in as /usr/lib would, pointing at a copy of rustype-linux under the
# sandbox (the vendor directory is merged with the user's by the systemctl stub).
sb_vendor_dropin() {
    mkdir -p "$SB/vendor/systemd/$RT_UNIT.d" "$SB/vendor/usr/libexec"
    cp "$RUSTYPE_LINUX_BIN" "$SB/vendor/usr/libexec/rustype-linux" || fail "cannot copy the vendor binary"
    printf '[Service]\nExecStart=\nExecStart=%s --own-session-name\nRestart=on-failure\nRestartSec=1\n' \
        "$SB/vendor/usr/libexec/rustype-linux" > "$SB/vendor/systemd/$RT_UNIT.d/rustype.conf"
}

sb_legacy_engine() {
    mkdir -p "$SB/home/.local/bin" "$SB/home/.local/lib/rustype"
    printf 'legacy\n' > "$SB/home/.local/bin/ibus-engine-rustype"
    printf 'legacy\n' > "$SB/home/.local/lib/rustype/ibus-engine-rustype"
}

# Copies the parts of the repository that install.sh, uninstall.sh and build-deb.sh use, so the
# scripts can run from a private tree. The real repository is only read.
sb_repo_copy() {
    local d="$SB/repo" dir
    mkdir -p "$d/rustype-core"
    cp -a "$RT_REPO_ROOT/install.sh" "$RT_REPO_ROOT/uninstall.sh" "$RT_REPO_ROOT/Cargo.toml" \
        "$RT_REPO_ROOT/Cargo.lock" "$d/" || fail "copying install scripts"
    cp -a "$RT_REPO_ROOT/rustype-core/Cargo.toml" "$d/rustype-core/" || fail "copying rustype-core manifest"
    for dir in assets tools gnome-shell-extension docs packaging; do
        cp -a "$RT_REPO_ROOT/$dir" "$d/" || fail "copying $dir"
    done
}

# Runs a command in the sandbox. Stdout and stderr go to $SB/out and $SB/err, the exit status to
# RT_RC. Fails the test when the run tried a command that is not available or a forbidden one.
sb_run() {
    local start
    start=$(wc -l < "$SB/calls.log")
    local -a base=(
        "PATH=$SB/stubs:$SB/allow"
        "HOME=$SB/home"
        "XDG_CONFIG_HOME=$SB/xdg/config"
        "XDG_DATA_HOME=$SB/xdg/data"
        "XDG_CACHE_HOME=$SB/xdg/cache"
        "XDG_RUNTIME_DIR=$SB/xdg/run"
        "XDG_CURRENT_DESKTOP=GNOME"
        "LANG=C.UTF-8"
        "LC_ALL=C.UTF-8"
        "BASH_ENV=$SB/bash_env"
        "IBUS_ADDRESS_FILE=$SB/state/ibus-address"
    )
    (
        umask 022
        cd "$SB_CWD" || exit 1
        exec env -i "${base[@]}" ${SB_EXTRA[@]+"${SB_EXTRA[@]}"} "$@"
    ) >"$SB/out" 2>"$SB/err" </dev/null
    RT_RC=$?

    tail -n +"$((start + 1))" "$SB/calls.log" > "$SB/new-calls"
    if grep -q '^NOTFOUND ' "$SB/new-calls"; then
        fail "command not found in the sandbox: $(grep -m1 '^NOTFOUND ' "$SB/new-calls")"
    fi
    # bash says "command not found", dash (/bin/sh) says "not found", env says "No such file".
    if grep -qE 'command not found|: not found$|env: .*No such file or directory' "$SB/err"; then
        fail "command not found in the sandbox: $(grep -m1 -E 'command not found|: not found$|env: .*No such file' "$SB/err")"
    fi
    if grep -E '^(sudo|apt|apt-get|pkill|killall|loginctl|ibus|dconf|dpkg)( |$)' "$SB/new-calls" \
        | grep -qv '^dpkg --print-architecture$'; then
        fail "forbidden command ran: $(grep -E '^(sudo|apt|apt-get|pkill|killall|loginctl|ibus|dconf|dpkg)( |$)' "$SB/new-calls" | head -n1)"
    fi
    return 0
}

# Architecture name the way build-deb.sh derives it when it has no dpkg.
rt_expected_arch() {
    case $(uname -m) in
        x86_64) echo amd64 ;;
        aarch64) echo arm64 ;;
        *) uname -m ;;
    esac
}

# ---- sandbox internals -----------------------------------------------------

_sb_start_ibus() {
    . "$SB/holder-lib.sh"
    start_holder "$SB/fake-ibus/ibus-daemon" || fail "cannot start the ibus stand-in"
    echo "$HOLDER_PID" > "$SB/state/mainpid"
    : > "$SB/state/active"
    printf 'IBUS_DAEMON_PID=%s\n' "$HOLDER_PID" > "$SB/state/ibus-address"
}

_sb_write_stubs() {
    cat > "$SB/ctl-lib.sh" <<'EOF'
# Sourced by every stub. Applies the first control line whose ERE matches "NAME ARGS": prints its
# output and exits with its status.
ctl_check() {
    local name=$1 line pat rc out
    shift
    line="$name${*:+ $*}"
    [ -f "$SB/ctl/$name" ] || return 0
    while IFS=$'\t' read -r pat rc out; do
        [ -n "$pat" ] || continue
        if [[ $line =~ $pat ]]; then
            printf '%b' "$out"
            exit "${rc:-0}"
        fi
    done < "$SB/ctl/$name"
    return 0
}
EOF

    cat > "$SB/stub-generic" <<'EOF'
#!/bin/bash
# Generic stub: logs the call and applies scripted results. Installed as stubs/NAME symlinks.
SB=${0%/stubs/*}
name=${0##*/}
printf '%s\n' "$name${*:+ $*}" >> "$SB/calls.log"
. "$SB/ctl-lib.sh"
ctl_check "$name" "$@"
exit 0
EOF
    chmod 755 "$SB/stub-generic"
    local n
    for n in sudo apt apt-get pkill killall loginctl ibus dconf glib-compile-schemas \
        update-desktop-database gtk-update-icon-cache gnome-shell gnome-extensions runuser; do
        ln -s "$SB/stub-generic" "$SB/stubs/$n"
    done

    cat > "$SB/stubs/systemctl" <<'EOF'
#!/bin/bash
SB=${0%/stubs/*}
st=$SB/state
UNIT=org.freedesktop.IBus.session.GNOME.service
TIMER=rustype-rollback.timer
printf '%s\n' "systemctl${*:+ $*}" >> "$SB/calls.log"
. "$SB/ctl-lib.sh"
ctl_check systemctl "$@"
. "$SB/holder-lib.sh"

# Starts the unit with the ExecStart that the drop-ins give (see effective_exe), or the stock
# ibus-daemon stand-in when there is none. The address file gets the new MainPID.
# The ExecStart systemd would run. Drop-ins from the vendor directory and the user directory are
# merged by file name in sorted order; a user file replaces a vendor file of the same name. An empty
# ExecStart= clears what came before, and the last ExecStart=/path wins.
effective_exe() {
    local vendor="$SB/vendor/systemd/$UNIT.d" user="$SB/xdg/config/systemd/user/$UNIT.d"
    local -A files=()
    local f name line exe=
    for f in "$vendor"/*.conf "$user"/*.conf; do
        [ -f "$f" ] || continue
        files[${f##*/}]=$f
    done
    for name in $(printf '%s\n' "${!files[@]}" | sort); do
        while IFS= read -r line; do
            case $line in
                ExecStart=) exe= ;;
                ExecStart=/*) exe=${line#ExecStart=}; exe=${exe%% *} ;;
            esac
        done < "${files[$name]}"
    done
    printf '%s' "$exe"
}

restart_unit() {
    local old exe
    if [ -s "$st/mainpid" ]; then
        old=$(cat "$st/mainpid")
        kill "$old" 2>/dev/null
    fi
    exe=$(effective_exe)
    if [ -n "$exe" ]; then
        [ -x "$exe" ] || return 1
        start_holder "$exe" || return 1
    else
        start_holder "$SB/fake-ibus/ibus-daemon" || return 1
    fi
    echo "$HOLDER_PID" > "$st/mainpid"
    printf 'IBUS_DAEMON_PID=%s\n' "$HOLDER_PID" > "${IBUS_ADDRESS_FILE:-$st/ibus-address}"
    : > "$st/active"
}

case "$*" in
    "--user cat "*) [ -e "$st/unit_present" ] || exit 1 ;;
    "--user show -p MainPID --value "*) cat "$st/mainpid" 2>/dev/null || echo 0 ;;
    "--user is-active --quiet $UNIT") [ -e "$st/active" ] || exit 1 ;;
    "--user is-active --quiet $TIMER") [ -e "$st/timer" ] || exit 1 ;;
    "--user stop $TIMER") rm -f "$st/timer" ;;
    "--user restart $UNIT"|"--user --no-block restart $UNIT") restart_unit || exit 1 ;;
esac
exit 0
EOF

    cat > "$SB/stubs/systemd-run" <<'EOF'
#!/bin/bash
# Arms a "timer": records the last argument (the rollback command) and marks the timer active.
SB=${0%/stubs/*}
printf '%s\n' "systemd-run${*:+ $*}" >> "$SB/calls.log"
. "$SB/ctl-lib.sh"
ctl_check systemd-run "$@"
printf '%s\n' "${@: -1}" > "$SB/state/timer_cmd"
: > "$SB/state/timer"
exit 0
EOF

    cat > "$SB/stubs/gsettings" <<'EOF'
#!/bin/bash
# Stateful for the input-sources key: "set" is kept and "get" returns it.
SB=${0%/stubs/*}
printf '%s\n' "gsettings${*:+ $*}" >> "$SB/calls.log"
. "$SB/ctl-lib.sh"
ctl_check gsettings "$@"
case "$1 $2 $3" in
    "get org.gnome.desktop.input-sources sources")
        if [ -e "$SB/state/sources" ]; then cat "$SB/state/sources"; else echo "@a(ss) []"; fi ;;
    "set org.gnome.desktop.input-sources sources")
        printf '%s\n' "$4" > "$SB/state/sources" ;;
    *) echo "gsettings stub: unexpected call: $*" >&2; exit 1 ;;
esac
exit 0
EOF

    cat > "$SB/stubs/getent" <<'EOF'
#!/bin/bash
# Looks up users in the sandbox passwd file ($SB/passwd) only.
SB=${0%/stubs/*}
printf '%s\n' "getent${*:+ $*}" >> "$SB/calls.log"
[ "$1" = passwd ] || exit 2
awk -F: -v u="$2" '$3 == u { print; found = 1; exit } END { exit !found }' "$SB/passwd"
EOF

    # timeout runs its command unless a control line says otherwise.
    cat > "$SB/stubs/timeout" <<'EOF'
#!/bin/bash
SB=${0%/stubs/*}
printf '%s\n' "timeout${*:+ $*}" >> "$SB/calls.log"
. "$SB/ctl-lib.sh"
ctl_check timeout "$@"
while [ $# -gt 0 ] && [ "${1#-}" != "$1" ]; do shift; done
shift
exec "$@"
EOF

    # cargo build copies the prebuilt binary into ./target/release, the path the scripts expect.
    cat > "$SB/stubs/cargo" <<'EOF'
#!/bin/bash
SB=${0%/stubs/*}
printf '%s\n' "cargo${*:+ $*}" >> "$SB/calls.log"
. "$SB/ctl-lib.sh"
ctl_check cargo "$@"
if [ "$1" = build ]; then
    mkdir -p target/release || exit 1
    cp -f "$(cat "$SB/state/bin-src")" target/release/rustype-linux || exit 1
    chmod 755 target/release/rustype-linux
fi
exit 0
EOF

    # dpkg answers only --print-architecture. Anything else (dpkg -i included) is logged and refused.
    cat > "$SB/stubs/dpkg" <<'EOF'
#!/bin/bash
SB=${0%/stubs/*}
printf '%s\n' "dpkg${*:+ $*}" >> "$SB/calls.log"
. "$SB/ctl-lib.sh"
ctl_check dpkg "$@"
if [ "$1" = --print-architecture ]; then
    case $(uname -m) in
        x86_64) echo amd64 ;;
        aarch64) echo arm64 ;;
        *) uname -m ;;
    esac
    exit 0
fi
echo "dpkg stub: refusing in the sandbox: $*" >&2
exit 1
EOF

    cat > "$SB/holder-lib.sh" <<'EOF'
# start_holder BIN: runs "BIN --feed" with a stdin that stays open, so the process keeps running as
# a stand-in for a daemon. Sets HOLDER_PID and records both processes in state/pids (killed at the
# end of the test).
start_holder() {
    local fifo="$SB/state/fifo.$RANDOM.$RANDOM"
    mkfifo "$fifo" || return 1
    ( exec sleep 600 > "$fifo" ) >/dev/null 2>&1 &
    echo "$!" >> "$SB/state/pids"
    ( exec "$1" --feed < "$fifo" ) >/dev/null 2>&1 &
    HOLDER_PID=$!
    echo "$HOLDER_PID" >> "$SB/state/pids"
}
EOF

    chmod 755 "$SB"/stubs/*
    cat > "$SB/bash_env" <<EOF
# BASH_ENV for every bash in the sandbox: reports commands that are neither stubbed nor allowed.
command_not_found_handle() {
    if grep -qxF -- "\$1" "$SB/allow-missing" 2>/dev/null; then
        return 127
    fi
    printf 'NOTFOUND %s\n' "\$1" >> "$SB/calls.log"
    echo "sandbox: \$1: command not found" >&2
    return 127
}
EOF
}

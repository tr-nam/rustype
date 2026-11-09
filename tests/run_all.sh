#!/usr/bin/env bash
# Runs the whole integration suite: install/uninstall, the .deb and maintainer scripts, and the
# settings tests. Exits nonzero when any part fails. No sudo, no running desktop and no GNOME
# install are needed: every test runs in a sandbox (tests/lib.sh).
#   RUSTYPE_LINUX_BIN=/abs/path/rustype-linux tests/run_all.sh
set -o pipefail
TESTS_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$TESTS_DIR/.." && pwd)
. "$TESTS_DIR/lib.sh"
rt_require_bin

RT_RESULTS_FILE=$(mktemp "${TMPDIR:-/tmp}/rustype-results.XXXXXX")
export RT_RESULTS_FILE
trap 'rm -f "$RT_RESULTS_FILE"' EXIT

suite_failures=0

run_suite() {
    local name=$1
    shift
    echo "==== $name"
    "$@"
    local rc=$?
    if [ "$rc" -ne 0 ]; then
        suite_failures=$((suite_failures + 1))
        echo "==== $name: FAILED (exit $rc)"
    else
        echo "==== $name: ok"
    fi
    echo
}

# The settings tests are written separately (tests/settings/). Run them in a sandbox too, from the
# repository root. A missing file is a failure, not a skip.
run_settings() (
    local script="$REPO_ROOT/tests/settings/test_settings.py"
    if [ ! -f "$script" ]; then
        echo "FAIL: $script does not exist, so the settings tests did not run"
        exit 1
    fi
    trap '_rt_cleanup' EXIT
    sb_new
    SB_CWD=$REPO_ROOT
    sb_env "RUSTYPE_LINUX_BIN=$RUSTYPE_LINUX_BIN"
    sb_run python3 tests/settings/test_settings.py
    local rc=$RT_RC
    cat "$SB/out"
    cat "$SB/err" >&2
    [ "$rc" -eq 0 ] || echo "settings tests exited with $rc"
    exit "$rc"
)

run_suite "install.sh / uninstall.sh" bash "$TESTS_DIR/install/run.sh"
run_suite "Debian package and maintainer scripts" bash "$TESTS_DIR/deb/run.sh"
run_suite "settings (tests/settings/test_settings.py)" run_settings

total_pass=0
total_fail=0
total_known=0
while read -r p f k; do
    total_pass=$((total_pass + p))
    total_fail=$((total_fail + f))
    total_known=$((total_known + k))
done < "$RT_RESULTS_FILE"

echo "INTEGRATION SUMMARY: passed=$total_pass failed=$((total_fail + suite_failures)) known-fail=$total_known"
[ $((total_fail + suite_failures)) -eq 0 ]

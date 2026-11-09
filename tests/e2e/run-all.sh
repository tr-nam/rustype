#!/usr/bin/env bash
# Runs every end-to-end case, one fresh session each, and prints every RESULT line. The real session's
# IBus unit and input sources are compared before and after. usage: run-all.sh ENGINE_BIN [CASES...]
# Each case is wrapped in timeout -k 5 150. Case e needs Google Chrome; without it e is SKIPPED.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
ENGINE=${1:?usage: run-all.sh ENGINE_BIN [CASES...]}; shift
CASES=${*:-a b c d e f g h}
# Per-case logs go to a writable temp dir: the scripts can be read-only (container image).
LOGDIR=$(mktemp -d /tmp/rustype-e2e-logs.XXXXXX) || exit 2
echo "logs: $LOGDIR"
snapshot() {
    { systemctl --user show -p MainPID,ActiveState org.freedesktop.IBus.session.GNOME.service 2>/dev/null
      gsettings get org.gnome.desktop.input-sources sources 2>/dev/null; } 2>&1 | tr '\n' ' '
}
BEFORE=$(snapshot)
echo "baseline before: $BEFORE"
FAILS=0
for c in $CASES; do
    if [ "$c" = e ] && [ ! -x /usr/bin/google-chrome ]; then
        echo "SKIP e: /usr/bin/google-chrome not present"; continue
    fi
    case "$c" in e) WORDS="Rus Traanf" ;; f) WORDS="1 2" ;; *) WORDS="-" ;; esac
    for w in $WORDS; do
        LOG="$LOGDIR/case-$c$([ "$w" = - ] || echo "-$w").log"
        t0=$(date +%s)
        if [ "$w" = - ]; then
            timeout -k 5 150 "$HERE/run-host.sh" "$ENGINE" "$c" > "$LOG" 2>&1
        else
            timeout -k 5 150 "$HERE/run-host.sh" "$ENGINE" "$c" "$w" > "$LOG" 2>&1
        fi
        rc=$?
        echo "case $c$([ "$w" = - ] || echo " ($w)") exit=$rc secs=$(( $(date +%s) - t0 ))"
        grep -E "^RESULT|^NOTE" "$LOG"
        [ "$rc" = 0 ] || FAILS=$((FAILS + 1))
    done
done
AFTER=$(snapshot)
echo "baseline after:  $AFTER"
if [ "$BEFORE" = "$AFTER" ]; then echo "baseline: IDENTICAL"; else echo "baseline: CHANGED"; FAILS=$((FAILS + 1)); fi
echo "failed runs: $FAILS"
exit $(( FAILS > 0 ))

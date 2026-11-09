#!/usr/bin/env bash
# Integration tests for install.sh and uninstall.sh. Each test runs a private copy of the scripts in
# a sandbox (tests/lib.sh). Nothing here touches the real session: no systemctl, gsettings or sudo
# reaches the host.
#   RUSTYPE_LINUX_BIN=/abs/path/rustype-linux tests/install/run.sh
set -o pipefail
. "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/lib.sh"
rt_require_bin

UNIT=org.freedesktop.IBus.session.GNOME.service
EXT=rustype-focus@namtr.dev
RESTART_RE='^systemctl --user restart org\.freedesktop\.IBus\.session\.GNOME\.service$'

dropin_path() { printf '%s' "$SB/xdg/config/systemd/user/$UNIT.d/zz-rustype.conf"; }
legacy_path() { printf '%s' "$SB/xdg/config/systemd/user/$UNIT.d/override.conf"; }

# An override.conf written by an older source install: it starts our binary, so it is ours.
write_legacy_ours() {
    mkdir -p "$SB/xdg/config/systemd/user/$UNIT.d"
    printf '[Service]\nExecStart=\nExecStart=%s --own-session-name\nRestart=on-failure\nRestartSec=1\n' \
        "$(installed_bin)" > "$(legacy_path)"
}
installed_bin() { printf '%s' "$SB/home/.local/lib/rustype/rustype-linux"; }

# Sandbox with the repo copy, the stock ibus stand-in running, and a health check that logs its
# result to calls.log. $1 is "ok" (default) or "fail".
setup_install() {
    sb_new
    sb_repo_copy
    if [ "${1:-ok}" = fail ]; then
        sb_env "RUSTYPE_HEALTH=printf 'health-fail\\n' >> '$SB/calls.log'; false"
    else
        sb_env "RUSTYPE_HEALTH=printf 'health-ok\\n' >> '$SB/calls.log'"
    fi
}

# Runs install.sh --yes and asserts it succeeded.
install_ok() {
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_zero "install.sh --yes"
}

# Runs uninstall.sh and asserts it succeeded.
uninstall_ok() {
    sb_run "$SB/repo/uninstall.sh"
    assert_rc_zero "uninstall.sh"
}

# ---- install.sh: order of events -------------------------------------------

t_happy_path_order() {
    setup_install ok
    sb_legacy_engine
    install_ok
    local arm restart health stop gset
    arm=$(log_line '^systemd-run ' 1)
    restart=$(log_line "$RESTART_RE" 1)
    health=$(log_line '^health-ok$' 1)
    # Two stops are expected: one inside arm_rollback (clears an old timer) and one after success.
    stop=$(log_line '^systemctl --user stop rustype-rollback\.timer$' last)
    gset=$(log_line '^gsettings set ' 1)
    [ -n "$arm" ] && [ -n "$restart" ] && [ -n "$health" ] && [ -n "$stop" ] && [ -n "$gset" ] \
        || fail "missing events: arm=$arm restart=$restart health=$health stop=$stop gsettings=$gset"
    assert_logged_count '^systemctl --user stop rustype-rollback\.timer$' 2
    [ "$arm" -lt "$restart" ] || fail "the rollback timer must be armed before the restart (arm=$arm restart=$restart)"
    [ "$restart" -lt "$health" ] || fail "the health check must run after the restart"
    [ "$health" -lt "$stop" ] || fail "the timer must be stopped only after the health check passed"
    [ "$stop" -lt "$gset" ] || fail "the input source must change after the timer is stopped"
    assert_absent "$SB/state/timer"
    assert_logged_count "$RESTART_RE" 1
    assert_absent "$SB/home/.local/bin/ibus-engine-rustype"
    assert_absent "$SB/home/.local/lib/rustype/ibus-engine-rustype"
}

t_health_failure_rolls_back() {
    setup_install fail
    sb_legacy_engine
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh with a failing health check"
    assert_logged '^health-fail$'
    assert_not_logged '^health-ok$'
    assert_absent "$(dropin_path)"
    assert_exists "$SB/home/.local/bin/ibus-engine-rustype"
    assert_exists "$SB/home/.local/lib/rustype/ibus-engine-rustype"
    assert_not_logged '^gsettings set '
    assert_logged_count "$RESTART_RE" 2
    local exe
    exe=$(readlink "/proc/$(cat "$SB/state/mainpid")/exe")
    assert_eq "${exe##*/}" ibus-daemon "unit MainPID after the rollback"
}

# A failed health check rolls back at once, so the timer must be gone: if it fired later it would
# restart the unit a second time.
t_health_failure_cancels_timer() {
    setup_install fail
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh with a failing health check"
    assert_absent "$SB/state/timer"
}

t_rollback_command_restores_stock() {
    setup_install fail
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh with a failing health check"
    assert_exists "$SB/state/timer_cmd"
    sb_run sh -c "$(cat "$SB/state/timer_cmd")"
    assert_rc_zero "the recorded rollback command"
    assert_logged_count "$RESTART_RE" 3
    assert_absent "$(dropin_path)"
}

t_arm_failure_leaves_no_dropin() {
    setup_install ok
    sb_ctl systemd-run '.*' 1
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh when the rollback cannot be armed"
    assert_not_logged '^systemctl --user restart '
    assert_absent "$(dropin_path)"
    assert_output_has "không hẹn được giờ"
}

t_build_failure_changes_nothing() {
    setup_install ok
    sb_ctl cargo ' build ' 1
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh when the build fails"
    assert_not_logged '^systemd-run '
    assert_not_logged '^systemctl --user restart '
    assert_absent "$(installed_bin)"
    assert_output_has "build lỗi"
}

# ---- install.sh: files, content, modes -------------------------------------

t_installed_files() {
    setup_install ok
    install_ok
    local bin h icon s
    bin=$(installed_bin)
    h=$SB/home/.local
    assert_mode "$bin" 755
    assert_same_file "$bin" "$RUSTYPE_LINUX_BIN"
    assert_link "$h/bin/rustype-linux" "$bin"
    assert_mode "$h/bin/rustype-settings" 755
    assert_same_file "$h/bin/rustype-settings" "$SB/repo/tools/rustype-settings"
    assert_mode "$h/share/applications/dev.namtr.RustypeSettings.desktop" 644
    assert_same_file "$h/share/applications/dev.namtr.RustypeSettings.desktop" \
        "$SB/repo/tools/dev.namtr.RustypeSettings.desktop"

    local f
    for f in "$SB/repo/gnome-shell-extension/$EXT"/*; do
        assert_same_file "$h/share/gnome-shell/extensions/$EXT/${f##*/}" "$f"
    done

    for s in 16 32 48 64 128 256 512; do
        icon="$h/share/icons/hicolor/${s}x${s}/apps"
        assert_mode "$icon/rustype.png" 644
        assert_same_file "$icon/rustype.png" "$SB/repo/assets/icons/${s}x${s}/rustype.png"
        assert_link "$icon/dev.namtr.RustypeSettings.png" rustype.png
    done
    icon="$h/share/icons/hicolor/scalable/apps"
    assert_mode "$icon/rustype.svg" 644
    assert_link "$icon/dev.namtr.RustypeSettings.svg" rustype.svg

    # The drop-in must be the packaged one with only the binary path changed.
    sed "s#/usr/libexec/rustype-linux#$bin#" "$SB/repo/packaging/systemd/$UNIT.d/rustype.conf" \
        > "$SB/expected-dropin"
    assert_same_file "$(dropin_path)" "$SB/expected-dropin"
    assert_mode "$(dropin_path)" 644
    printf '[Service]\nExecStart=\nExecStart=%s --own-session-name\nRestart=on-failure\nRestartSec=1\n' "$bin" \
        > "$SB/expected-literal"
    assert_same_file "$(dropin_path)" "$SB/expected-literal"

    assert_exists "$SB/xdg/config/rustype/config"
    assert_logged '^gtk-update-icon-cache -q -t -f '
    assert_logged '^update-desktop-database '
}

t_reinstall_is_idempotent() {
    setup_install ok
    install_ok
    printf '# my settings\ninput_method = keep-me\n' >> "$SB/xdg/config/rustype/config"
    cp "$SB/xdg/config/rustype/config" "$SB/config-1"
    cp "$(dropin_path)" "$SB/dropin-1"

    install_ok
    assert_same_file "$SB/xdg/config/rustype/config" "$SB/config-1"
    assert_same_file "$(dropin_path)" "$SB/dropin-1"
    assert_logged_count '^gsettings set ' 1
    assert_logged_count "$RESTART_RE" 2
    local n
    n=$(grep -o "('ibus', 'rustype')" "$SB/state/sources" | wc -l)
    assert_eq "$n" 1 "rustype entries in the input sources"
}

t_sources_insert_after_first_entry() {
    setup_install ok
    sb_set_sources "[('xkb', 'us'), ('xkb', 'vn')]"
    install_ok
    assert_text "$SB/state/sources" "[('xkb', 'us'), ('ibus', 'rustype'), ('xkb', 'vn')]
"
}

t_sources_insert_into_empty_list() {
    setup_install ok
    sb_set_sources "@a(ss) []"
    install_ok
    assert_text "$SB/state/sources" "[('ibus', 'rustype')]
"
}

t_refuses_outside_gnome() {
    setup_install ok
    sb_env "XDG_CURRENT_DESKTOP=KDE"
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh on KDE"
    assert_output_has "không phải phiên GNOME"
    assert_not_logged '^cargo '
    assert_not_logged '^systemd-run '
    assert_absent "$(installed_bin)"
}

t_refuses_without_terminal_or_yes() {
    setup_install ok
    sb_run "$SB/repo/install.sh"
    assert_rc_nonzero "install.sh with no terminal and no --yes"
    assert_output_has "không có terminal"
    assert_not_logged '^cargo '
}

t_refuses_when_unit_missing() {
    setup_install ok
    sb_ctl systemctl '--user cat ' 1
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh without the IBus unit"
    assert_output_has "không tìm thấy unit"
    assert_not_logged '^cargo '
}

t_rejects_bad_argument() {
    setup_install ok
    sb_run "$SB/repo/install.sh" --bogus
    assert_rc_nonzero "install.sh --bogus"
    assert_output_has "tham số không hợp lệ"
}

# ---- install.sh: tolerated failures ----------------------------------------

t_extension_installed_and_enabled() {
    setup_install ok
    install_ok
    assert_exists "$SB/home/.local/share/gnome-shell/extensions/$EXT/metadata.json"
    assert_logged "^gnome-extensions enable $EXT\$"
}

t_extension_skipped_without_gnome_shell() {
    setup_install ok
    sb_remove_stub gnome-shell
    install_ok
    assert_absent "$SB/home/.local/share/gnome-shell/extensions/$EXT"
    assert_not_logged '^gnome-extensions '
}

t_extension_enable_failure_is_tolerated() {
    setup_install ok
    sb_ctl gnome-extensions ' enable ' 1
    install_ok
    assert_exists "$SB/home/.local/share/gnome-shell/extensions/$EXT/metadata.json"
}

t_icon_cache_failure_is_tolerated() {
    setup_install ok
    sb_ctl gtk-update-icon-cache '.*' 1
    install_ok
    assert_exists "$SB/home/.local/share/icons/hicolor/48x48/apps/rustype.png"
}

t_missing_desktop_database_tool_is_tolerated() {
    setup_install ok
    sb_remove_stub update-desktop-database
    sb_expect_missing update-desktop-database
    install_ok
    assert_exists "$SB/home/.local/share/applications/dev.namtr.RustypeSettings.desktop"
}

t_extension_copied_without_gnome_extensions_tool() {
    setup_install ok
    sb_remove_stub gnome-extensions
    install_ok
    assert_exists "$SB/home/.local/share/gnome-shell/extensions/$EXT/metadata.json"
}

# install.sh must not exit when the input sources cannot be read: it warns, skips only that step
# and runs the rest (the extension, icons, settings app and the --doctor check).
assert_later_steps_ran() {
    assert_exists "$SB/home/.local/share/gnome-shell/extensions/$EXT/metadata.json"
    assert_exists "$SB/home/.local/bin/rustype-settings"
    assert_exists "$SB/home/.local/share/icons/hicolor/48x48/apps/rustype.png"
    assert_output_has "rustype-linux --doctor"
    assert_output_has "Xong."
}

t_gsettings_missing_warns_and_continues() {
    setup_install ok
    sb_remove_stub gsettings
    sb_expect_missing gsettings
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_zero "install.sh without gsettings"
    assert_output_has "không đọc được nguồn nhập"
    assert_output_has "Input Sources"
    assert_not_logged '^gsettings '
    assert_later_steps_ran
}

t_doctor_failure_is_reported_not_fatal() {
    setup_install ok
    sb_ctl timeout '--doctor' 3
    install_ok
    assert_output_has "thoát với mã lỗi 3"
}

t_gsettings_read_failure_warns_and_continues() {
    setup_install ok
    sb_ctl gsettings ' get ' 1
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_zero "install.sh when gsettings cannot read the sources"
    assert_output_has "không đọc được nguồn nhập"
    assert_output_has "Input Sources"
    assert_not_logged '^gsettings set '
    assert_logged_count "$RESTART_RE" 1
    assert_later_steps_ran
}

# ---- packaged drop-in and the drop-in names --------------------------------
# Regression: the source drop-in used to be override.conf. systemd merges drop-ins by name and the
# packaged rustype.conf sorts after override.conf, so the package binary started and the health
# check failed. With the vendor drop-in present, the run below must succeed.

t_install_succeeds_with_packaged_dropin() {
    setup_install ok
    sb_vendor_dropin
    install_ok
    assert_exists "$(dropin_path)"
    assert_eq "$(readlink "/proc/$(cat "$SB/state/mainpid")/exe")" "$(installed_bin)" \
        "the unit must run the source install's binary"
}

t_failed_health_with_packaged_dropin_rolls_back_to_packaged_binary() {
    setup_install fail
    sb_vendor_dropin
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh with a failing health check"
    assert_absent "$(dropin_path)"
    assert_output_has "Đã hoàn tác: $SB/vendor/usr/libexec/rustype-linux đang chạy"
    assert_output_lacks "CẢNH BÁO: sau khi hoàn tác"
}

t_old_override_of_ours_removed_after_success() {
    setup_install ok
    write_legacy_ours
    install_ok
    assert_absent "$(legacy_path)"
    assert_exists "$(dropin_path)"
}

t_old_override_of_ours_kept_after_failed_health() {
    setup_install fail
    write_legacy_ours
    sb_run "$SB/repo/install.sh" --yes
    assert_rc_nonzero "install.sh with a failing health check"
    assert_exists "$(legacy_path)"
    assert_absent "$(dropin_path)"
}

t_foreign_override_untouched_by_install_and_uninstall() {
    setup_install ok
    mkdir -p "$SB/xdg/config/systemd/user/$UNIT.d"
    printf '[Service]\nEnvironment=FOO=1\n' > "$(legacy_path)"
    cp "$(legacy_path)" "$SB/foreign-expected"
    install_ok
    assert_same_file "$(legacy_path)" "$SB/foreign-expected"
    uninstall_ok
    assert_same_file "$(legacy_path)" "$SB/foreign-expected"
    assert_absent "$(dropin_path)"
}

t_uninstall_removes_both_of_ours() {
    setup_install ok
    install_ok
    write_legacy_ours
    uninstall_ok
    assert_absent "$(dropin_path)"
    assert_absent "$(legacy_path)"
}

# ---- uninstall.sh ----------------------------------------------------------

t_uninstall_removes_install_and_keeps_config() {
    setup_install ok
    sb_set_sources "[('xkb', 'us'), ('xkb', 'vn')]"
    install_ok
    uninstall_ok
    local h=$SB/home/.local
    assert_absent "$(dirname "$(installed_bin)")"
    assert_absent "$h/bin/rustype-linux"
    assert_absent "$h/bin/rustype-settings"
    assert_absent "$h/share/applications/dev.namtr.RustypeSettings.desktop"
    assert_absent "$h/share/gnome-shell/extensions/$EXT"
    assert_absent "$h/share/icons/hicolor/scalable/apps/rustype.svg"
    assert_absent "$h/share/icons/hicolor/48x48/apps/rustype.png"
    assert_absent "$(dropin_path)"
    assert_exists "$SB/xdg/config/rustype/config"
    assert_text "$SB/state/sources" "[('xkb', 'us'), ('xkb', 'vn')]
"
    assert_before 'systemctl --user daemon-reload' "$RESTART_RE"
    local pid exe
    pid=$(cat "$SB/state/mainpid")
    exe=$(readlink "/proc/$pid/exe")
    assert_eq "${exe##*/}" ibus-daemon "unit MainPID after uninstall"
}

t_uninstall_without_install_does_not_restart() {
    sb_new
    sb_repo_copy
    sb_run "$SB/repo/uninstall.sh"
    assert_rc_zero "uninstall.sh on a machine without Rustype"
    assert_not_logged "$RESTART_RE"
}

t_uninstall_tolerates_timer_stop_failure() {
    setup_install ok
    install_ok
    sb_ctl systemctl 'stop rustype-rollback' 1
    uninstall_ok
    assert_absent "$(installed_bin)"
}

# A failing daemon-reload must not stop uninstall.sh (set -e): it warns and keeps cleaning up.
t_uninstall_continues_after_daemon_reload_failure() {
    setup_install ok
    install_ok
    sb_ctl systemctl 'daemon-reload' 1
    sb_run "$SB/repo/uninstall.sh"
    assert_rc_zero "uninstall.sh when daemon-reload fails"
    assert_output_has "daemon-reload thất bại"
    assert_absent "$(installed_bin)"
    assert_absent "$SB/home/.local/share/gnome-shell/extensions/$EXT"
}

# A failing restart must not stop uninstall.sh either; the ibus-daemon wait is skipped.
t_uninstall_continues_after_restart_failure() {
    setup_install ok
    install_ok
    sb_ctl systemctl '--user restart org' 1
    sb_run "$SB/repo/uninstall.sh"
    assert_rc_zero "uninstall.sh when restart fails"
    assert_output_has "restart thất bại"
    assert_absent "$(installed_bin)"
    assert_absent "$SB/home/.local/share/gnome-shell/extensions/$EXT"
}

t_uninstall_sudo_not_used_without_old_component() {
    if [ -e /usr/share/ibus/component/rustype.xml ]; then
        skip_test "host has /usr/share/ibus/component/rustype.xml; the sudo path cannot be redirected"
    fi
    setup_install ok
    install_ok
    uninstall_ok
    assert_not_logged '^sudo '
}

test_case "happy path: arm, restart, health, stop timer, then the legacy engine" t_happy_path_order
test_case "failed health check: nonzero exit, legacy engine kept, drop-in removed, stock unit back" t_health_failure_rolls_back
test_case "failed health check: the immediate rollback cancels the timer" t_health_failure_cancels_timer
test_case "the recorded rollback command restores the stock unit" t_rollback_command_restores_stock
test_case "rollback timer cannot be armed: no restart, no drop-in left" t_arm_failure_leaves_no_dropin
test_case "build failure: nothing is armed, restarted or installed" t_build_failure_changes_nothing
test_case "installed files, modes, links, icons and the drop-in text" t_installed_files
test_case "reinstall on top of an install is idempotent and keeps user config" t_reinstall_is_idempotent
test_case "input sources: new entry goes after the first one" t_sources_insert_after_first_entry
test_case "input sources: new entry into an empty list" t_sources_insert_into_empty_list
test_case "refuses outside a GNOME session" t_refuses_outside_gnome
test_case "refuses without a terminal and without --yes" t_refuses_without_terminal_or_yes
test_case "refuses when the IBus unit does not exist" t_refuses_when_unit_missing
test_case "rejects an unknown argument" t_rejects_bad_argument
test_case "install succeeds with the packaged rustype.conf present (zz-rustype.conf sorts after it)" t_install_succeeds_with_packaged_dropin
test_case "failed health check with the packaged drop-in rolls back to the packaged binary, no false warning" t_failed_health_with_packaged_dropin_rolls_back_to_packaged_binary
test_case "old override.conf of ours is removed after a successful install" t_old_override_of_ours_removed_after_success
test_case "old override.conf of ours is kept after a failed health check" t_old_override_of_ours_kept_after_failed_health
test_case "a foreign override.conf is untouched by install and uninstall" t_foreign_override_untouched_by_install_and_uninstall
test_case "uninstall removes both drop-ins of ours" t_uninstall_removes_both_of_ours
test_case "extension copied and enabled when gnome-shell exists" t_extension_installed_and_enabled
test_case "extension skipped without gnome-shell" t_extension_skipped_without_gnome_shell
test_case "gnome-extensions enable failure is tolerated" t_extension_enable_failure_is_tolerated
test_case "gtk-update-icon-cache failure is tolerated" t_icon_cache_failure_is_tolerated
test_case "missing update-desktop-database is tolerated" t_missing_desktop_database_tool_is_tolerated
test_case "extension is copied even without the gnome-extensions tool" t_extension_copied_without_gnome_extensions_tool
test_case "missing gsettings: warns, skips only the source step, finishes" t_gsettings_missing_warns_and_continues
test_case "rustype-linux --doctor failure is reported, not fatal" t_doctor_failure_is_reported_not_fatal
test_case "gsettings read failure: warns, skips only the source step, finishes" t_gsettings_read_failure_warns_and_continues
test_case "uninstall removes the install, keeps config, restores the input sources" t_uninstall_removes_install_and_keeps_config
test_case "uninstall without an install does not restart the unit" t_uninstall_without_install_does_not_restart
test_case "uninstall tolerates a failed timer stop" t_uninstall_tolerates_timer_stop_failure
test_case "uninstall keeps cleaning up when daemon-reload fails (warns, exit 0)" t_uninstall_continues_after_daemon_reload_failure
test_case "uninstall keeps cleaning up when restart fails (warns, exit 0)" t_uninstall_continues_after_restart_failure
test_case "uninstall does not call sudo when no old component file exists" t_uninstall_sudo_not_used_without_old_component

rt_summary

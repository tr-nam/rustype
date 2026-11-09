#!/usr/bin/env bash
# Integration tests for the Debian package: packaging/build-deb.sh and the maintainer scripts.
# build-deb.sh runs on a private copy of the repository (it writes target/ under its root), with a
# stubbed cargo that copies RUSTYPE_LINUX_BIN in. dpkg-deb builds and inspects the package. dpkg
# itself is not runnable here (the stub refuses everything except --print-architecture).
#   RUSTYPE_LINUX_BIN=/abs/path/rustype-linux tests/deb/run.sh
set -o pipefail
. "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/lib.sh"
rt_require_bin

UNIT=org.freedesktop.IBus.session.GNOME.service
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$RT_REPO_ROOT/rustype-core/Cargo.toml" | head -n1)
ARCH=$(rt_expected_arch)

# Builds the package in a sandbox. Sets DEB to the result.
deb_build() {
    sb_new
    sb_repo_copy
    sb_run "$SB/repo/packaging/build-deb.sh"
    assert_rc_zero "build-deb.sh"
    DEB="$SB/repo/target/packages/rustype_${VERSION}-1_${ARCH}.deb"
    assert_exists "$DEB"
    assert_same_file "$SB/repo/target/release/rustype-linux" "$RUSTYPE_LINUX_BIN"
}

# Writes the package listing (dpkg-deb -c) to $SB/list.txt.
deb_list() {
    sb_run dpkg-deb -c "$DEB"
    assert_rc_zero "dpkg-deb -c"
    cp "$SB/out" "$SB/list.txt"
}

# Mode string (for example -rwxr-xr-x) of a packaged path, from the listing.
deb_mode() {
    awk -v p="./$1" '{ for (i = 1; i <= NF; i++) if ($i == p) { print $1; exit } }' "$SB/list.txt"
}

# Extracts the package to $SB/x. Sets X to that directory.
deb_extract() {
    X="$SB/x"
    sb_run dpkg-deb -x "$DEB" "$X"
    assert_rc_zero "dpkg-deb -x"
}

# Extracts the maintainer scripts to $SB/ctl (dpkg-deb -e), as dpkg does at install time.
deb_scripts() {
    sb_run dpkg-deb -e "$DEB" "$SB/ctl"
    assert_rc_zero "dpkg-deb -e"
    assert_mode "$SB/ctl/postinst" 755
    assert_mode "$SB/ctl/prerm" 755
    assert_mode "$SB/ctl/postrm" 755
}

# ---- build and control -----------------------------------------------------

t_build_produces_package() {
    deb_build
    deb_list
    assert_logged '^cargo build --release --locked -p rustype-linux$'
    assert_not_logged '^dpkg-deb -i|^dpkg -i'
}

t_control_fields() {
    deb_build
    sb_run dpkg-deb -f "$DEB" Package Version Architecture Depends Maintainer
    assert_rc_zero "dpkg-deb -f"
    local f
    f=$(cat "$SB/out")
    assert_contains "$f" "Package: rustype" "control"
    assert_contains "$f" "Version: ${VERSION}-1" "control: version must match the workspace"
    assert_contains "$f" "Architecture: $ARCH" "control"
    assert_contains "$f" "ibus (>= 1.5.0)" "control: Depends"
    assert_contains "$f" "gnome-shell" "control: Depends"
    assert_contains "$f" "python3-gi" "control: Depends"
    assert_contains "$f" "gir1.2-gtk-4.0" "control: Depends"
    assert_contains "$f" "gir1.2-adw-1" "control: Depends"
}

t_workspace_versions_agree() {
    local rl cl top
    rl=$(sed -n 's/^version = "\(.*\)"/\1/p' "$RT_REPO_ROOT/rustype-linux/Cargo.toml" | head -n1)
    cl=$(sed -n 's/^version = "\(.*\)"/\1/p' "$RT_REPO_ROOT/rustype-core/Cargo.toml" | head -n1)
    top=$(head -n1 "$RT_REPO_ROOT/packaging/debian/changelog" | sed -n 's/^rustype (\(.*\)) .*/\1/p')
    assert_eq "$rl" "$VERSION" "rustype-linux version"
    assert_eq "$top" "${VERSION}-1" "newest debian/changelog entry"
}

# ---- package contents ------------------------------------------------------

t_package_contents_and_modes() {
    deb_build
    deb_list
    local expect path mode
    expect='usr/libexec/rustype-linux -rwxr-xr-x
usr/bin/rustype-linux lrwxrwxrwx
usr/bin/rustype-settings -rwxr-xr-x
usr/share/applications/dev.namtr.RustypeSettings.desktop -rw-r--r--
usr/lib/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/rustype.conf -rw-r--r--
etc/xdg/autostart/dev.namtr.RustypeActivate.desktop -rw-r--r--
usr/share/gnome-shell/extensions/rustype-focus@namtr.dev/extension.js -rw-r--r--
usr/share/gnome-shell/extensions/rustype-focus@namtr.dev/metadata.json -rw-r--r--
usr/share/man/man1/rustype-linux.1.gz -rw-r--r--
usr/share/man/man1/rustype-settings.1.gz -rw-r--r--
usr/share/icons/hicolor/scalable/apps/rustype.svg -rw-r--r--
usr/share/icons/hicolor/48x48/apps/rustype.png -rw-r--r--
usr/share/icons/hicolor/48x48/apps/dev.namtr.RustypeSettings.png lrwxrwxrwx
usr/share/doc/rustype/copyright -rw-r--r--
usr/share/doc/rustype/changelog.Debian.gz -rw-r--r--'
    while read -r path mode; do
        assert_eq "$(deb_mode "$path")" "$mode" "mode of /$path in the package"
    done <<< "$expect"
    local s
    for s in 16 32 48 64 128 256 512; do
        assert_eq "$(deb_mode "usr/share/icons/hicolor/${s}x${s}/apps/rustype.png")" "-rw-r--r--" "icon ${s}px"
    done
    assert_eq "$(deb_mode "usr/bin/rustype-linux")" "lrwxrwxrwx" "/usr/bin link"
}

t_package_has_no_legacy_ibus_component() {
    deb_build
    deb_list
    if grep -qE 'ibus-engine-rustype|rustype\.xml' "$SB/list.txt"; then
        fail "the package still ships the old IBus component: $(grep -E 'ibus-engine-rustype|rustype\.xml' "$SB/list.txt" | head -n1)"
    fi
}

t_packaged_binary_is_the_built_binary() {
    deb_build
    deb_extract
    assert_same_file "$X/usr/libexec/rustype-linux" "$RUSTYPE_LINUX_BIN"
}

t_packaged_dropin_text() {
    deb_build
    deb_extract
    printf '[Service]\nExecStart=\nExecStart=/usr/libexec/rustype-linux --own-session-name\nRestart=on-failure\nRestartSec=1\n' \
        > "$SB/expected-dropin"
    assert_same_file "$X/usr/lib/systemd/user/$UNIT.d/rustype.conf" "$SB/expected-dropin"
}

t_packaged_dropin_matches_makefile() {
    # `make install` writes its drop-in with printf. The packaged file must say the same thing.
    local mk
    mk=$(grep -F 'ExecStart=$(PREFIX)/libexec/rustype-linux --own-session-name' "$RT_REPO_ROOT/Makefile")
    [ -n "$mk" ] || fail "Makefile no longer writes the drop-in with the expected ExecStart"
    grep -qxF 'ExecStart=/usr/libexec/rustype-linux --own-session-name' \
        "$RT_REPO_ROOT/packaging/systemd/$UNIT.d/rustype.conf" \
        || fail "packaging drop-in ExecStart differs from the Makefile"
}

# ---- maintainer scripts ----------------------------------------------------

# Adds a passwd entry for NAME with UID, used by the getent stub.
add_user() {
    printf '%s:x:%s:%s::/home/%s:/bin/sh\n' "$1" "$2" "$2" "$1" >> "$SB/passwd"
}

# Creates the user's bus socket path under the fake runtime root.
add_bus() {
    mkdir -p "$SB/run/user/$1"
    : > "$SB/run/user/$1/bus"
}

# The exact command line the scripts hand to runuser for a user. Args: NAME UID, then the program.
runuser_line() {
    local name=$1 uid=$2
    shift 2
    printf 'runuser -u %s -- env XDG_RUNTIME_DIR=%s DBUS_SESSION_BUS_ADDRESS=unix:path=%s/bus timeout 20 %s\n' \
        "$name" "$SB/run/user/$uid" "$SB/run/user/$uid" "$*"
}

# Builds the package, extracts its maintainer scripts and points RUSTYPE_RUNTIME_ROOT at the fake
# /run/user. Users are added by the tests.
deb_scripts_setup() {
    deb_build
    deb_scripts
    sb_env "RUSTYPE_RUNTIME_ROOT=$SB/run/user"
}

t_postinst_without_users() {
    deb_scripts_setup
    sb_run "$SB/ctl/postinst" configure
    assert_rc_zero "postinst configure with no users"
    assert_not_logged '^runuser '
    assert_output_has "lần đăng nhập GNOME tiếp theo"
    assert_line_logged "update-desktop-database -q /usr/share/applications"
    assert_line_logged "gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor"
}

t_postinst_activates_one_user() {
    deb_scripts_setup
    add_user alice 1000
    add_bus 1000
    sb_run "$SB/ctl/postinst" configure
    assert_rc_zero "postinst configure"
    assert_line_logged "$(runuser_line alice 1000 /usr/libexec/rustype-linux --activate --restart)"
    assert_logged_count '^runuser ' 1
    assert_output_has "đã sẵn sàng"
}

t_postinst_selects_only_interactive_users_with_a_bus() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    add_user bob 1001;   add_bus 1001
    add_user sysd 999;   add_bus 999
    add_bus 1002                      # bus, but no passwd entry
    add_user dave 1003                # passwd entry, no bus
    sb_run "$SB/ctl/postinst" configure
    assert_rc_zero "postinst configure"
    assert_line_logged "$(runuser_line alice 1000 /usr/libexec/rustype-linux --activate --restart)"
    assert_line_logged "$(runuser_line bob 1001 /usr/libexec/rustype-linux --activate --restart)"
    assert_logged_count '^runuser ' 2
}

t_postinst_activation_failure_does_not_fail_package() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    add_user bob 1001;   add_bus 1001
    sb_ctl runuser ' -u alice .*--activate' 1
    sb_run "$SB/ctl/postinst" configure
    assert_rc_zero "postinst when activation fails for one user"
    assert_line_logged "$(runuser_line bob 1001 /usr/libexec/rustype-linux --activate --restart)"
}

t_postinst_activation_timeout_does_not_fail_package() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    add_user bob 1001;   add_bus 1001
    sb_ctl runuser ' -u alice .*--activate' 124
    sb_run "$SB/ctl/postinst" configure
    assert_rc_zero "postinst when activation times out for one user"
    assert_line_logged "$(runuser_line bob 1001 /usr/libexec/rustype-linux --activate --restart)"
}

# Rustype is reported ready only when activation succeeded; with no success, the next-login message.
t_postinst_no_ready_message_after_failed_activation() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    sb_ctl runuser ' -u alice .*--activate' 1
    sb_run "$SB/ctl/postinst" configure
    assert_rc_zero "postinst configure"
    assert_output_has "lần đăng nhập GNOME tiếp theo"
    if grep -q "đã sẵn sàng" "$SB/out"; then
        fail "postinst says Rustype is ready although activation failed"
    fi
}

t_postinst_one_fails_one_succeeds_reports_ready() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    add_user bob 1001;   add_bus 1001
    sb_ctl runuser ' -u alice .*--activate' 1
    sb_run "$SB/ctl/postinst" configure
    assert_rc_zero "postinst when one user fails and another succeeds"
    assert_line_logged "$(runuser_line alice 1000 /usr/libexec/rustype-linux --activate --restart)"
    assert_line_logged "$(runuser_line bob 1001 /usr/libexec/rustype-linux --activate --restart)"
    assert_output_has "đã sẵn sàng"
}

t_prerm_remove_deactivates_users() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    add_user bob 1001;   add_bus 1001
    add_user sysd 999;   add_bus 999
    sb_run "$SB/ctl/prerm" remove
    assert_rc_zero "prerm remove"
    assert_line_logged "$(runuser_line alice 1000 /usr/libexec/rustype-linux --deactivate)"
    assert_line_logged "$(runuser_line bob 1001 /usr/libexec/rustype-linux --deactivate)"
    assert_logged_count '^runuser ' 2
}

t_prerm_upgrade_does_nothing() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    sb_run "$SB/ctl/prerm" upgrade
    assert_rc_zero "prerm upgrade"
    assert_not_logged '^runuser '
}

t_prerm_deactivate_failure_is_tolerated() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    sb_ctl runuser ' --deactivate' 1
    sb_run "$SB/ctl/prerm" remove
    assert_rc_zero "prerm remove when deactivation fails"
}

t_postrm_remove_reloads_then_restarts() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    add_user bob 1001;   add_bus 1001
    sb_run "$SB/ctl/postrm" remove
    assert_rc_zero "postrm remove"
    local reload restart
    reload=$(runuser_line alice 1000 systemctl --user daemon-reload)
    restart=$(runuser_line alice 1000 systemctl --user --no-block restart "$UNIT")
    assert_line_logged "$reload"
    assert_line_logged "$restart"
    local rl tl
    rl=$(grep -nxF -- "$reload" "$SB/calls.log" | head -n1 | cut -d: -f1)
    tl=$(grep -nxF -- "$restart" "$SB/calls.log" | head -n1 | cut -d: -f1)
    [ "$rl" -lt "$tl" ] || fail "daemon-reload (line $rl) must run before the restart (line $tl)"
    assert_line_logged "$(runuser_line bob 1001 systemctl --user daemon-reload)"
    assert_line_logged "update-desktop-database -q /usr/share/applications"
}

t_postrm_purge_same_as_remove() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    sb_run "$SB/ctl/postrm" purge
    assert_rc_zero "postrm purge"
    assert_line_logged "$(runuser_line alice 1000 systemctl --user --no-block restart "$UNIT")"
}

t_postrm_upgrade_does_nothing() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    sb_run "$SB/ctl/postrm" upgrade
    assert_rc_zero "postrm upgrade"
    assert_not_logged '^runuser '
    assert_not_logged '^update-desktop-database '
}

t_postrm_failures_are_tolerated() {
    deb_scripts_setup
    add_user alice 1000; add_bus 1000
    sb_ctl runuser 'daemon-reload|restart' 124
    sb_run "$SB/ctl/postrm" remove
    assert_rc_zero "postrm remove when every user-level command times out"
}

t_maintainer_scripts_reject_unknown_action() {
    deb_scripts_setup
    sb_run "$SB/ctl/postinst" frobnicate
    assert_rc_nonzero "postinst frobnicate"
    sb_run "$SB/ctl/postrm" frobnicate
    assert_rc_nonzero "postrm frobnicate"
}

assert_contains() {
    case $1 in
        *"$2"*) ;;
        *) fail "$3: [$2] not found in: $1" ;;
    esac
}

test_case "build-deb.sh builds the package from a private copy" t_build_produces_package
test_case "control fields: name, version, architecture, dependencies" t_control_fields
test_case "rustype-linux, rustype-core and the changelog agree on the version" t_workspace_versions_agree
test_case "package contents and modes" t_package_contents_and_modes
test_case "package ships no old IBus component" t_package_has_no_legacy_ibus_component
test_case "packaged binary is the built rustype-linux" t_packaged_binary_is_the_built_binary
test_case "packaged systemd drop-in has the exact expected text" t_packaged_dropin_text
test_case "packaged drop-in agrees with the Makefile" t_packaged_dropin_matches_makefile
test_case "postinst with no users only prints the next-login message" t_postinst_without_users
test_case "postinst activates one user with a bus" t_postinst_activates_one_user
test_case "postinst picks only interactive users that have a bus" t_postinst_selects_only_interactive_users_with_a_bus
test_case "postinst: a failed activation does not fail the package" t_postinst_activation_failure_does_not_fail_package
test_case "postinst: a timed-out activation does not fail the package" t_postinst_activation_timeout_does_not_fail_package
test_case "postinst: no 'ready' message when activation failed" t_postinst_no_ready_message_after_failed_activation
test_case "postinst: one user fails, another succeeds, reports ready" t_postinst_one_fails_one_succeeds_reports_ready
test_case "prerm remove deactivates each user" t_prerm_remove_deactivates_users
test_case "prerm upgrade does nothing" t_prerm_upgrade_does_nothing
test_case "prerm: a failed deactivation is tolerated" t_prerm_deactivate_failure_is_tolerated
test_case "postrm remove: daemon-reload before restart, per user" t_postrm_remove_reloads_then_restarts
test_case "postrm purge: same as remove" t_postrm_purge_same_as_remove
test_case "postrm upgrade does nothing" t_postrm_upgrade_does_nothing
test_case "postrm: failed user-level commands are tolerated" t_postrm_failures_are_tolerated
test_case "maintainer scripts reject unknown actions" t_maintainer_scripts_reject_unknown_action

rt_summary

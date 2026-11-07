#!/usr/bin/env bash
# Cài Rustype cho người dùng hiện tại:
#   build -> chép engine vào ~/.local/lib/rustype -> đăng ký với IBus -> thêm vào nguồn nhập GNOME.
# Chỉ bước chép file component vào /usr/share/ibus/component cần sudo, và chỉ khi file đó đổi.
# Chạy lại script này sau mỗi lần sửa code để cập nhật.
set -euo pipefail
cd "$(dirname "$0")"

CARGO="$(command -v cargo || echo "$HOME/.cargo/bin/cargo")"
DEST="$HOME/.local/lib/rustype"
BIN="$DEST/ibus-engine-rustype"
XML=/usr/share/ibus/component/rustype.xml

echo "==> Build"
"$CARGO" build --release --quiet

echo "==> Chép engine vào $BIN"
install -Dm755 target/release/ibus-engine-rustype "$BIN.new"
mv -f "$BIN.new" "$BIN"
"$BIN" --write-config

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
"$BIN" --xml > "$tmp"
if ! cmp -s "$tmp" "$XML"; then
    echo "==> Đăng ký với IBus (cần mật khẩu sudo)"
    sudo install -m644 "$tmp" "$XML"
fi

echo "==> Khởi động lại IBus"
ibus restart >/dev/null 2>&1 || true
sleep 2

sources="$(gsettings get org.gnome.desktop.input-sources sources)"
if [[ "$sources" != *"('ibus', 'rustype')"* ]]; then
    echo "==> Thêm Rustype vào nguồn nhập của GNOME"
    new="$(python3 -I -c '
import ast, sys
s = sys.argv[1].replace("@a(ss) ", "")
items = ast.literal_eval(s) if s.strip() != "[]" else []
items.insert(1 if items else 0, ("ibus", "rustype"))
print("[" + ", ".join("(%r, %r)" % t for t in items) + "]")
' "$sources")"
    gsettings set org.gnome.desktop.input-sources sources "$new"
fi

EXT_SRC="gnome-shell-extension/rustype-focus@namtr.dev"
EXT_DEST="$HOME/.local/share/gnome-shell/extensions/rustype-focus@namtr.dev"
if [[ -d "$EXT_SRC" ]] && command -v gnome-shell >/dev/null 2>&1; then
    echo "==> Cài đặt GNOME Shell extension (báo focus cho app_modes)"
    mkdir -p "$EXT_DEST"
    cp -f "$EXT_SRC"/* "$EXT_DEST/"
    if command -v gnome-extensions >/dev/null 2>&1; then
        gnome-extensions enable "rustype-focus@namtr.dev" >/dev/null 2>&1 || true
    fi
fi

echo "==> Cài đặt biểu tượng (icons)"
ICON_BASE="$HOME/.local/share/icons/hicolor"
mkdir -p "$ICON_BASE/scalable/apps" "$ICON_BASE/symbolic/apps"
install -m644 assets/icons/scalable/rustype.svg "$ICON_BASE/scalable/apps/rustype.svg"
install -m644 assets/icons/symbolic/rustype-symbolic.svg "$ICON_BASE/symbolic/apps/rustype-symbolic.svg"
for s in 16 32 48 64 128 256 512; do
    mkdir -p "$ICON_BASE/${s}x${s}/apps"
    install -m644 "assets/icons/${s}x${s}/rustype.png" "$ICON_BASE/${s}x${s}/apps/rustype.png"
done
gtk-update-icon-cache -q -t -f "$ICON_BASE" 2>/dev/null || true

echo "==> Cài đặt ứng dụng Cài đặt (rustype-settings)"
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
install -m755 tools/rustype-settings "$HOME/.local/bin/rustype-settings"
install -m644 tools/rustype-settings.desktop "$HOME/.local/share/applications/rustype-settings.desktop"

echo
echo "Xong. Nhấn Super+Space để chuyển sang Rustype."
echo "Cấu hình: ~/.config/rustype/config"

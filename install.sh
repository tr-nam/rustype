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

echo
echo "Xong. Nhấn Super+Space để chuyển sang Rustype (biểu tượng VN trên thanh trên cùng)."
echo "Cấu hình: ~/.config/rustype/config"

#!/usr/bin/env bash
# Gỡ Rustype. Giữ lại ~/.config/rustype để lần sau cài lại không mất cấu hình.
set -euo pipefail

sources="$(gsettings get org.gnome.desktop.input-sources sources)"
if [[ "$sources" == *"('ibus', 'rustype')"* ]]; then
    new="$(python3 -I -c '
import ast, sys
items = [t for t in ast.literal_eval(sys.argv[1]) if t != ("ibus", "rustype")]
print("[" + ", ".join("(%r, %r)" % t for t in items) + "]" if items else "@a(ss) []")
' "$sources")"
    gsettings set org.gnome.desktop.input-sources sources "$new"
fi

if [[ -e /usr/share/ibus/component/rustype.xml ]]; then
    sudo rm -f /usr/share/ibus/component/rustype.xml
fi
ibus restart >/dev/null 2>&1 || true
rm -rf "$HOME/.local/lib/rustype"
echo "Đã gỡ Rustype. Cấu hình vẫn ở ~/.config/rustype (xóa tay nếu muốn)."

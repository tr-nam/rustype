#!/usr/bin/env bash
# Build standard Debian/Ubuntu .deb package for Rustype
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

VERSION="${1:-${VERSION:-$(grep -m1 '^version = ' "$ROOT_DIR/rustype-core/Cargo.toml" | cut -d'"' -f2)}}"
VERSION="${VERSION:-0.1.0}"
REVISION="1"

# Determine Debian architecture
if command -v dpkg >/dev/null 2>&1; then
    ARCH="$(dpkg --print-architecture)"
else
    UNAME_M="$(uname -m)"
    case "$UNAME_M" in
        x86_64)  ARCH="amd64" ;;
        aarch64) ARCH="arm64" ;;
        armv7l)  ARCH="armhf" ;;
        riscv64) ARCH="riscv64" ;;
        *)       ARCH="$UNAME_M" ;;
    esac
fi

echo "===================================================="
echo " Building Rustype .deb package"
echo " Version:      ${VERSION}-${REVISION}"
echo " Architecture: ${ARCH}"
echo "===================================================="

# 1. Build release binaries
echo "==> [1/6] Biên dịch mã nguồn release với Cargo..."
CARGO="${CARGO:-cargo}"
"$CARGO" build --release --locked

# 2. Chuẩn bị thư mục staging
STAGING="$ROOT_DIR/target/deb-staging"
OUTPUT_DIR="$ROOT_DIR/target/packages"
rm -rf "$STAGING"
mkdir -p "$STAGING" "$OUTPUT_DIR"

# 3. Tạo cấu trúc thư mục FHS
echo "==> [2/6] Tạo cấu trúc thư mục FHS..."
mkdir -p "$STAGING/usr/libexec" \
         "$STAGING/usr/bin" \
         "$STAGING/usr/share/ibus/component" \
         "$STAGING/usr/share/applications" \
         "$STAGING/usr/share/gnome-shell/extensions/rustype-focus@namtr.dev" \
         "$STAGING/usr/share/man/man1" \
         "$STAGING/usr/share/doc/rustype" \
         "$STAGING/DEBIAN"

# 4. Sao chép và phân quyền nhị phân & script
echo "==> [3/6] Cài đặt file thực thi và tài nguyên..."
install -m755 "$ROOT_DIR/target/release/ibus-engine-rustype" "$STAGING/usr/libexec/ibus-engine-rustype"
ln -sf ../libexec/ibus-engine-rustype "$STAGING/usr/bin/ibus-engine-rustype"
install -m755 "$ROOT_DIR/tools/rustype-settings" "$STAGING/usr/bin/rustype-settings"
install -m644 "$ROOT_DIR/tools/dev.namtr.RustypeSettings.desktop" "$STAGING/usr/share/applications/dev.namtr.RustypeSettings.desktop"

# IBus Component XML trỏ vào /usr/libexec
cat <<EOF > "$STAGING/usr/share/ibus/component/rustype.xml"
<?xml version="1.0" encoding="utf-8"?>
<component>
    <name>org.freedesktop.IBus.Rustype</name>
    <description>Rustype - bộ gõ tiếng Việt Telex &amp; VNI siêu nhẹ</description>
    <exec>/usr/libexec/ibus-engine-rustype --ibus</exec>
    <version>${VERSION}</version>
    <author>Nam Trần</author>
    <license>MIT</license>
    <homepage>https://namtr.dev</homepage>
    <textdomain>rustype</textdomain>
    <engines>
        <engine>
            <name>rustype</name>
            <language>vi</language>
            <license>MIT</license>
            <author>Nam Trần</author>
            <icon>rustype</icon>
            <layout>us</layout>
            <longname>Rustype</longname>
            <description>Bộ gõ siêu nhẹ bằng Rust cho Linux</description>
            <rank>99</rank>
            <symbol>R</symbol>
        </engine>
    </engines>
</component>
EOF
chmod 644 "$STAGING/usr/share/ibus/component/rustype.xml"

# Cài đặt biểu tượng icon theme
for s in 16 32 48 64 128 256 512; do
    mkdir -p "$STAGING/usr/share/icons/hicolor/${s}x${s}/apps"
    install -m644 "$ROOT_DIR/assets/icons/${s}x${s}/rustype.png" "$STAGING/usr/share/icons/hicolor/${s}x${s}/apps/rustype.png"
    ln -sf rustype.png "$STAGING/usr/share/icons/hicolor/${s}x${s}/apps/dev.namtr.RustypeSettings.png"
done
mkdir -p "$STAGING/usr/share/icons/hicolor/scalable/apps"
install -m644 "$ROOT_DIR/assets/icons/scalable/rustype.svg" "$STAGING/usr/share/icons/hicolor/scalable/apps/rustype.svg"
ln -sf rustype.svg "$STAGING/usr/share/icons/hicolor/scalable/apps/dev.namtr.RustypeSettings.svg"

# GNOME Shell extension
cp -a "$ROOT_DIR/gnome-shell-extension/rustype-focus@namtr.dev/"* "$STAGING/usr/share/gnome-shell/extensions/rustype-focus@namtr.dev/"

# Man pages nén gzip
echo "==> [4/6] Nén Man pages và tài liệu..."
gzip -9n -c "$ROOT_DIR/docs/man/ibus-engine-rustype.1" > "$STAGING/usr/share/man/man1/ibus-engine-rustype.1.gz"
gzip -9n -c "$ROOT_DIR/docs/man/rustype-settings.1" > "$STAGING/usr/share/man/man1/rustype-settings.1.gz"
chmod 644 "$STAGING/usr/share/man/man1/"*.gz

# Docs & Copyright
install -m644 "$ROOT_DIR/packaging/debian/copyright" "$STAGING/usr/share/doc/rustype/copyright"
gzip -9n -c "$ROOT_DIR/packaging/debian/changelog" > "$STAGING/usr/share/doc/rustype/changelog.Debian.gz"
chmod 644 "$STAGING/usr/share/doc/rustype/"*

# 5. Thiết lập metadata DEBIAN
echo "==> [5/6] Thiết lập control metadata..."
INSTALLED_SIZE="$(du -ks "$STAGING" | cut -f1)"

# Sinh control file với đúng kích thước và kiến trúc
sed -e "s/^Architecture: .*/Architecture: ${ARCH}/" \
    -e "s/^Version: .*/Version: ${VERSION}-${REVISION}/" \
    "$ROOT_DIR/packaging/debian/control" > "$STAGING/DEBIAN/control"
echo "Installed-Size: ${INSTALLED_SIZE}" >> "$STAGING/DEBIAN/control"

install -m755 "$ROOT_DIR/packaging/debian/postinst" "$STAGING/DEBIAN/postinst"
install -m755 "$ROOT_DIR/packaging/debian/postrm" "$STAGING/DEBIAN/postrm"

# Chuẩn hóa quyền thư mục và tập tin theo chuẩn Debian
find "$STAGING" -type d -exec chmod 755 {} +
find "$STAGING" -type f ! -perm -111 -exec chmod 644 {} +

# 6. Đóng gói .deb
DEB_FILE="$OUTPUT_DIR/rustype_${VERSION}-${REVISION}_${ARCH}.deb"
echo "==> [6/6] Đóng gói thành phẩm qua dpkg-deb..."
dpkg-deb --build --root-owner-group "$STAGING" "$DEB_FILE"

echo
echo "===================================================="
echo " Đóng gói thành công!"
echo " Gói đầu ra: $DEB_FILE"
echo " Dung lượng: $(du -h "$DEB_FILE" | cut -f1)"
echo "===================================================="
echo

# Hiển thị thông tin gói vừa tạo
dpkg-deb -I "$DEB_FILE"

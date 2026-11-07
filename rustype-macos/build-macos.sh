#!/usr/bin/env bash
set -euo pipefail

# Build script for Rustype macOS Input Method Bundle (Rustype.app)
# Requires macOS with Xcode Command Line Tools.

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$DIR/.." && pwd)"
APP_DIR="$DIR/Rustype.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

echo "==> [1/3] Biên dịch rustype-core (staticlib)..."
cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --package rustype-core --release

echo "==> [2/3] Chuẩn bị App Bundle..."
rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$DIR/Info.plist" "$CONTENTS_DIR/Info.plist"

echo "==> [3/3] Biên dịch Objective-C InputMethodKit wrapper với clang..."
clang -O3 \
    -framework Cocoa \
    -framework InputMethodKit \
    -I"$ROOT_DIR/rustype-core/include" \
    -L"$ROOT_DIR/target/release" \
    -lrustype_core \
    "$DIR/main.m" "$DIR/RustypeInputController.m" \
    -o "$MACOS_DIR/Rustype"

echo "==> Đóng gói thành công: $APP_DIR"
echo "Để cài đặt thử nghiệm, chạy:"
echo "  cp -R \"$APP_DIR\" ~/Library/Input\\ Methods/"

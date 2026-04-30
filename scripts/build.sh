#!/usr/bin/env bash
# Build and ad-hoc sign gmaps.app.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "▶ cargo bundle --release"
cargo bundle --release

APP_PATH="$(pwd)/target/release/bundle/osx/gmaps.app"
BINARY_PATH="$APP_PATH/Contents/MacOS/gmaps"

echo ""
echo "▶ Ad-hoc code signing"
codesign --force --deep --sign - "$APP_PATH"

echo ""
echo "▶ Validating Info.plist"
plutil -lint "$APP_PATH/Contents/Info.plist"

echo ""
echo "▶ Verifying signature"
codesign --verify --verbose "$APP_PATH" 2>&1 | head -5

echo ""
echo "✓ Build complete"
echo "  App:    $APP_PATH"
echo "  Binary: $BINARY_PATH"
echo ""
echo "── To install as a global 'gmaps' command ──"
echo "  mkdir -p ~/.local/bin"
echo "  ln -sf \"$BINARY_PATH\" ~/.local/bin/gmaps"
echo "  # If ~/.local/bin is not on PATH, add this to your shell rc file:"
echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""

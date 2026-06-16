#!/usr/bin/env bash
# Build, Developer ID sign, notarize, and staple gmaps.app for distribution.
#
# Unlike scripts/build.sh (ad-hoc signing, for local dev), this produces a
# Gatekeeper-acceptable bundle that other users can run without a one-time
# right-click → Open bypass.
#
# Requirements (all already expected in your environment):
#   - A "Developer ID Application" certificate in the login keychain.
#   - APPLE_ID                    Apple ID email used for notarization.
#   - APPLE_TEAM_ID               10-char Apple Developer Team ID.
#   - APPLE_APP_SPECIFIC_PASSWORD App-specific password (NOT your Apple ID
#                                 password); create at appleid.apple.com.
#
# Optional overrides:
#   - SIGN_IDENTITY  codesign identity to use. Defaults to the unique
#                    "Developer ID Application" entry in the keychain.
set -euo pipefail

cd "$(dirname "$0")/.."

SIGN_IDENTITY="${SIGN_IDENTITY:-Developer ID Application}"

# Fail early with a clear message if a credential is missing, without echoing
# the value itself.
: "${APPLE_ID:?APPLE_ID is not set (export it before running)}"
: "${APPLE_TEAM_ID:?APPLE_TEAM_ID is not set (export it before running)}"
: "${APPLE_APP_SPECIFIC_PASSWORD:?APPLE_APP_SPECIFIC_PASSWORD is not set (export it before running)}"

echo "▶ cargo bundle --release"
cargo bundle --release

APP_PATH="$(pwd)/target/release/bundle/osx/gmaps.app"
BINARY_PATH="$APP_PATH/Contents/MacOS/gmaps"

echo ""
echo "▶ Developer ID signing (hardened runtime + secure timestamp)"
echo "  identity: ${SIGN_IDENTITY}"
# Sign inner-out: the executable first, then the bundle. (--deep is
# deprecated and unreliable; sign each component explicitly.) Hardened
# runtime (--options runtime) and a secure timestamp are both required for
# notarization to succeed.
codesign --force --options runtime --timestamp \
  --sign "$SIGN_IDENTITY" "$BINARY_PATH"
codesign --force --options runtime --timestamp \
  --sign "$SIGN_IDENTITY" "$APP_PATH"

echo ""
echo "▶ Validating Info.plist"
plutil -lint "$APP_PATH/Contents/Info.plist"

echo ""
echo "▶ Verifying signature"
codesign --verify --strict --verbose=2 "$APP_PATH"

ARCH="$(uname -m)"
VERSION="$(grep -m1 '^version' Cargo.toml | awk -F'"' '{print $2}')"
ZIP_NAME="gmaps-${VERSION}-macos-${ARCH}.app.zip"

echo ""
echo "▶ Zipping for notarization (${ZIP_NAME})"
# ditto preserves the signature and extended attributes; zip -r does not.
rm -f "$ZIP_NAME"
ditto -c -k --keepParent "$APP_PATH" "$ZIP_NAME"

echo ""
echo "▶ Submitting to Apple notary service (this can take a few minutes)"
xcrun notarytool submit "$ZIP_NAME" \
  --apple-id "$APPLE_ID" \
  --team-id "$APPLE_TEAM_ID" \
  --password "$APPLE_APP_SPECIFIC_PASSWORD" \
  --wait

echo ""
echo "▶ Stapling the notarization ticket onto the bundle"
xcrun stapler staple "$APP_PATH"
xcrun stapler validate "$APP_PATH"

echo ""
echo "▶ Gatekeeper assessment"
spctl --assess --type exec --verbose=4 "$APP_PATH" || true

echo ""
echo "▶ Re-zipping the stapled bundle for distribution"
rm -f "$ZIP_NAME"
ditto -c -k --keepParent "$APP_PATH" "$ZIP_NAME"
SHA256="$(shasum -a 256 "$ZIP_NAME" | awk '{print $1}')"

echo ""
echo "✓ Signed, notarized, and stapled"
echo "  App:    $APP_PATH"
echo "  Zip:    $(pwd)/$ZIP_NAME"
echo "  SHA256: $SHA256"

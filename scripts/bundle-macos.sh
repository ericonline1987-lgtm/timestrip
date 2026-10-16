#!/bin/bash
# Builds TimeStrip.app.
#
#   scripts/bundle-macos.sh              build for this Mac
#   scripts/bundle-macos.sh --universal  build for Apple Silicon + Intel
#   scripts/bundle-macos.sh --install    also copy to /Applications and launch it
set -euo pipefail
cd "$(dirname "$0")/.."

UNIVERSAL=0
INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --universal) UNIVERSAL=1 ;;
    --install) INSTALL=1 ;;
    *) echo "unknown option: $arg" >&2; exit 1 ;;
  esac
done

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
OUT=target/bundle
APP="$OUT/TimeStrip.app"

if [[ $UNIVERSAL == 1 ]]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  BIN="$OUT/timestrip-universal"
  mkdir -p "$OUT"
  lipo -create -output "$BIN" \
    target/aarch64-apple-darwin/release/timestrip \
    target/x86_64-apple-darwin/release/timestrip
else
  cargo build --release
  BIN=target/release/timestrip
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/TimeStrip"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>TimeStrip</string>
    <key>CFBundleDisplayName</key><string>TimeStrip</string>
    <key>CFBundleIdentifier</key><string>com.timestrip.app</string>
    <key>CFBundleExecutable</key><string>TimeStrip</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleVersion</key><string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSCalendarsFullAccessUsageDescription</key><string>TimeStrip shows your calendar events in the time of each region you follow.</string>
    <key>NSCalendarsUsageDescription</key><string>TimeStrip shows your calendar events in the time of each region you follow.</string>
</dict>
</plist>
PLIST

# Ad-hoc signature (no Apple developer account needed).
codesign --force --deep --sign - "$APP"
echo "Built $APP"

if [[ $INSTALL == 1 ]]; then
  pkill -x TimeStrip 2>/dev/null || true
  rm -rf /Applications/TimeStrip.app
  cp -R "$APP" /Applications/
  open /Applications/TimeStrip.app
  echo "Installed and launched /Applications/TimeStrip.app"
fi

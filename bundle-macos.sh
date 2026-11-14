#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_DIR="$SCRIPT_DIR/dist-macos/Pi Agent.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

export PATH="$HOME/.cargo/bin:$PATH"

echo "[pi-gpui] Building release binary..."
(cd "$SCRIPT_DIR" && cargo build --release)

rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$SCRIPT_DIR/target/release/pi-gpui" "$MACOS_DIR/pi-gpui"
chmod +x "$MACOS_DIR/pi-gpui"

if [[ -f "$SCRIPT_DIR/assets/icon.icns" ]]; then
  cp "$SCRIPT_DIR/assets/icon.icns" "$RESOURCES_DIR/icon.icns"
elif [[ -f "$REPO_ROOT/src-tauri/icons/icon.icns" ]]; then
  cp "$REPO_ROOT/src-tauri/icons/icon.icns" "$RESOURCES_DIR/icon.icns"
fi

if [[ -f "$SCRIPT_DIR/dist-sidecar/server.mjs" || -f "$SCRIPT_DIR/dist-sidecar/server.cjs" ]]; then
  mkdir -p "$RESOURCES_DIR/sidecar"
  cp "$SCRIPT_DIR/sidecar/api-sidecar.mjs" "$RESOURCES_DIR/sidecar/"
  cp "$SCRIPT_DIR/sidecar/router-utils.mjs" "$RESOURCES_DIR/sidecar/"
  [[ -f "$SCRIPT_DIR/dist-sidecar/server.mjs" ]] && cp "$SCRIPT_DIR/dist-sidecar/server.mjs" "$RESOURCES_DIR/sidecar/"
  [[ -f "$SCRIPT_DIR/dist-sidecar/server.cjs" ]] && cp "$SCRIPT_DIR/dist-sidecar/server.cjs" "$RESOURCES_DIR/sidecar/"
fi

cat > "$CONTENTS_DIR/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>pi-gpui</string>
  <key>CFBundleIconFile</key>
  <string>icon.icns</string>
  <key>CFBundleIdentifier</key>
  <string>works.earendil.pi-gpui</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>PiCrust</string>
  <key>CFBundleDisplayName</key>
  <string>PiCrust (派壳)</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0.0</string>
  <key>CFBundleVersion</key>
  <string>1.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP_DIR"
codesign --verify --deep --strict "$APP_DIR"

echo "[pi-gpui] macOS app bundle ready:"
du -sh "$APP_DIR"
ls -lh "$MACOS_DIR/pi-gpui"

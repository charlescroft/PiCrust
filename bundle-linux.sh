#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPDIR="$SCRIPT_DIR/dist-linux/PiCrust.AppDir"
OUTDIR="$SCRIPT_DIR/dist-linux"

echo "[picrust] 1. Compiling PiCrust (Release x86_64-unknown-linux-gnu)..."
(cd "$SCRIPT_DIR" && cargo build --release)

echo "[picrust] 2. Preparing AppDir layout..."
rm -rf "$APPDIR"
mkdir -p "$OUTDIR"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/lib" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
         "$APPDIR/usr/resources/sidecar"

cp "$SCRIPT_DIR/target/release/PiCrust" "$APPDIR/usr/bin/PiCrust"
chmod +x "$APPDIR/usr/bin/PiCrust"

if [ -f "$SCRIPT_DIR/assets/icon.png" ]; then
  cp "$SCRIPT_DIR/assets/icon.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/picrust.png"
  cp "$SCRIPT_DIR/assets/icon.png" "$APPDIR/picrust.png"
fi

if [ -f "$SCRIPT_DIR/dist-sidecar/server.mjs" ]; then
  cp "$SCRIPT_DIR/dist-sidecar/server.mjs" "$APPDIR/usr/resources/sidecar/"
fi
if [ -f "$SCRIPT_DIR/sidecar/api-sidecar.mjs" ]; then
  cp "$SCRIPT_DIR/sidecar/api-sidecar.mjs" "$APPDIR/usr/resources/sidecar/"
  cp "$SCRIPT_DIR/sidecar/router-utils.mjs" "$APPDIR/usr/resources/sidecar/"
fi

cat > "$APPDIR/picrust.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=PiCrust
GenericName=AI Coding Workbench
Comment=The crispy, ultra-fast, GPU-native Rust crust baked for Pi Agent
Exec=PiCrust %U
Icon=picrust
Terminal=false
Categories=Development;IDE;
StartupWMClass=picrust
EOF
cp "$APPDIR/picrust.desktop" "$APPDIR/usr/share/applications/"

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
set -e
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS}"
exec "${HERE}/usr/bin/PiCrust" "$@"
EOF
chmod +x "$APPDIR/AppRun"

echo "[picrust] 3. Building standalone AppImage..."
if [ ! -f "$OUTDIR/appimagetool" ]; then
  curl -L -s -o "$OUTDIR/appimagetool" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$OUTDIR/appimagetool"
fi

ARCH=x86_64 "$OUTDIR/appimagetool" --appimage-extract-and-run "$APPDIR" "$OUTDIR/PiCrust-x86_64.AppImage"
echo "[picrust] Standalone AppImage ready: $OUTDIR/PiCrust-x86_64.AppImage"

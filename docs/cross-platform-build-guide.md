# 🌐 PiCrust Cross-Platform Packaging & Distribution Guide

This document provides a comprehensive, battle-tested guide for compiling and packaging **PiCrust** for **Windows (`.exe` / NSIS / portable ZIP)** and **Linux (Standalone AppImage)**.

---

## 🪟 Part 1: Windows Distribution (`x86_64-pc-windows-msvc`)

### 1.1 Architecture & Prerequisites
GPUI on Windows relies on **Direct3D 11/12** for hardware acceleration, **DirectWrite** for font rendering, and **Windows Media Foundation**.

#### Build Machine Requirements:
- **OS**: Windows 10/11 (x64 / ARM64)
- **Rust Toolchain**: `stable-x86_64-pc-windows-msvc`
  ```cmd
  rustup target add x86_64-pc-windows-msvc
  ```
- **Visual Studio 2022 / Build Tools**:
  - Workload: *Desktop development with C++*
  - Windows 10 or 11 SDK (version `10.0.22000.0` or higher)

---

### 1.2 Windows Application Manifest & Icon Embedding

GPUI requires high-DPI awareness (Per-Monitor V2) and long path support. Create `pi-gpui/build.rs` or configure `winres`:

#### `pi-gpui/app.manifest`:
```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="works.earendil.picrust" version="1.0.0.0" processorArchitecture="*"/>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <!-- Per-Monitor V2 DPI Awareness -->
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">True/PM</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
      <!-- Long Path Support -->
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
      <!-- UTF-8 Active Code Page -->
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 and Windows 11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
```

#### Embed via `winres` in `Cargo.toml`:
```toml
[target.'cfg(target_os = "windows")'.build-dependencies]
winres = "0.1"
```

And in `build.rs`:
```rust
fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set_manifest_file("app.manifest");
        let _ = res.compile();
    }
}
```

---

### 1.3 Windows Compilation & Packaging Script (`bundle-windows.ps1`)

```powershell
# bundle-windows.ps1
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$OutputDir = Join-Path $ScriptDir "dist-windows\PiCrust"

Write-Host "==> Compiling PiCrust (Release x86_64-pc-windows-msvc)..." -ForegroundColor Cyan
cargo build --release --target x86_64-pc-windows-msvc

if (Test-Path $OutputDir) { Remove-Item -Recurse -Force $OutputDir }
New-Item -ItemType Directory -Path $OutputDir | Out-Null
New-Item -ItemType Directory -Path "$OutputDir\resources\sidecar" | Out-Null

# Copy binary
Copy-Item "$ScriptDir\target\x86_64-pc-windows-msvc\release\pi-gpui.exe" "$OutputDir\PiCrust.exe"

# Copy Sidecar Assets
Copy-Item "$ScriptDir\sidecar\api-sidecar.mjs" "$OutputDir\resources\sidecar\"
Copy-Item "$ScriptDir\sidecar\router-utils.mjs" "$OutputDir\resources\sidecar\"
if (Test-Path "$ScriptDir\dist-sidecar\server.mjs") {
    Copy-Item "$ScriptDir\dist-sidecar\server.mjs" "$OutputDir\resources\sidecar\"
}

# Create Portable ZIP distribution
Write-Host "==> Creating PiCrust-v1.0.0-windows-x64.zip..." -ForegroundColor Green
Compress-Archive -Path "$OutputDir\*" -DestinationPath "$ScriptDir\dist-windows\PiCrust-v1.0.0-windows-x64.zip" -Force

Write-Host "Windows build complete!" -ForegroundColor Green
```

---

### 1.4 NSIS Installer Script (`installer.nsi`)

Use [NSIS](https://nsis.sourceforge.io/) to produce a production-grade setup installer:

```nsis
!include "MUI2.nsh"

Name "PiCrust"
OutFile "dist-windows\PiCrust-v1.0.0-Setup.exe"
InstallDir "$LOCALAPPDATA\Programs\PiCrust"
RequestExecutionLevel user

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Section "Install"
  SetOutPath "$INSTDIR"
  File /r "dist-windows\PiCrust\*"
  
  CreateDirectory "$SMPROGRAMS\PiCrust"
  CreateShortcut "$SMPROGRAMS\PiCrust\PiCrust.lnk" "$INSTDIR\PiCrust.exe" "" "$INSTDIR\PiCrust.exe" 0
  CreateShortcut "$DESKTOP\PiCrust.lnk" "$INSTDIR\PiCrust.exe" "" "$INSTDIR\PiCrust.exe" 0
  
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\PiCrust.lnk"
  RMDir /r "$SMPROGRAMS\PiCrust"
  RMDir /r "$INSTDIR"
SectionEnd
```

---

## 🐧 Part 2: Linux AppImage (`x86_64-unknown-linux-gnu`)

### 2.1 Architecture & Prerequisites
GPUI on Linux operates on **Vulkan** for rendering (with automatic CPU llvmpipe fallback), **Wayland** & **X11 / XCB** for display, and **fontconfig / freetype** for typography.

#### Build Container / Environment:
Always build on **Ubuntu 20.04 or 22.04 LTS** (never on cutting-edge rolling distros like Arch) to keep `glibc` requirements low (`>= 2.31`), ensuring the AppImage runs on Debian, Fedora, openSUSE, Ubuntu, Arch, and RedHat.

```bash
sudo apt update && sudo apt install -y \
  build-essential \
  pkg-config \
  libasound2-dev \
  libfontconfig1-dev \
  libfreetype6-dev \
  libx11-dev \
  libx11-xcb-dev \
  libxcb-xfixes0-dev \
  libxcb-shape0-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  libwayland-dev \
  libvulkan-dev \
  vulkan-tools \
  libssl-dev
```

---

### 2.2 AppDir Directory Layout

An AppImage is an ISO9660 container mounted at runtime by `AppRun`:

```text
PiCrust.AppDir/
├── AppRun                      # Executable symlink or wrapper script
├── picrust.desktop             # Desktop entry specification
├── picrust.png                 # Application icon (256x256)
└── usr/
    ├── bin/
    │   └── picrust             # Rust compiled binary
    ├── lib/                    # Bundled shared libraries (libc.so excluded)
    └── share/
        ├── applications/
        │   └── picrust.desktop
        └── icons/hicolor/256x256/apps/
            └── picrust.png
```

---

### 2.3 Desktop Entry (`picrust.desktop`)

```ini
[Desktop Entry]
Type=Application
Name=PiCrust
GenericName=AI Coding Workbench
Comment=The crispy, ultra-fast, GPU-native Rust crust for Pi Agent
Exec=picrust %U
Icon=picrust
Terminal=false
Categories=Development;IDE;TextEditor;
StartupWMClass=picrust
MimeType=text/plain;
```

---

### 2.4 AppRun Launch Wrapper (`AppRun`)

Create an executable shell script `AppRun` in the root of `AppDir`:

```bash
#!/bin/sh
set -e

HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS}"

# GPU / Wayland fallback enforcement for GPUI
if [ -z "${WAYLAND_DISPLAY}" ] && [ -n "${DISPLAY}" ]; then
  export GDK_BACKEND=x11
fi

exec "${HERE}/usr/bin/picrust" "$@"
```

---

### 2.5 Automated Linux AppImage Packaging Script (`bundle-linux.sh`)

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPDIR="$SCRIPT_DIR/dist-linux/PiCrust.AppDir"
OUTDIR="$SCRIPT_DIR/dist-linux"

echo "==> 1. Compiling PiCrust (Release x86_64-unknown-linux-gnu)..."
cargo build --release

echo "==> 2. Preparing AppDir structure..."
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps" "$APPDIR/usr/resources/sidecar"

# Copy binary & assets
cp "$SCRIPT_DIR/target/release/pi-gpui" "$APPDIR/usr/bin/picrust"
chmod +x "$APPDIR/usr/bin/picrust"

cp "$SCRIPT_DIR/assets/icon.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/picrust.png"
cp "$SCRIPT_DIR/assets/icon.png" "$APPDIR/picrust.png"

# Copy sidecar files
if [ -f "$SCRIPT_DIR/dist-sidecar/server.mjs" ]; then
  cp "$SCRIPT_DIR/sidecar/api-sidecar.mjs" "$APPDIR/usr/resources/sidecar/"
  cp "$SCRIPT_DIR/sidecar/router-utils.mjs" "$APPDIR/usr/resources/sidecar/"
  cp "$SCRIPT_DIR/dist-sidecar/server.mjs" "$APPDIR/usr/resources/sidecar/"
fi

# Create Desktop file
cat > "$APPDIR/picrust.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=PiCrust
GenericName=AI Coding Workbench
Comment=The crispy, ultra-fast, GPU-native Rust crust for Pi Agent
Exec=picrust %U
Icon=picrust
Terminal=false
Categories=Development;IDE;
StartupWMClass=picrust
EOF
cp "$APPDIR/picrust.desktop" "$APPDIR/usr/share/applications/"

# Create AppRun
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
set -e
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS}"
exec "${HERE}/usr/bin/picrust" "$@"
EOF
chmod +x "$APPDIR/AppRun"

echo "==> 3. Downloading appimagetool if absent..."
if [ ! -f "$OUTDIR/appimagetool" ]; then
  curl -L -o "$OUTDIR/appimagetool" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$OUTDIR/appimagetool"
fi

echo "==> 4. Bundling standalone AppImage..."
ARCH=x86_64 "$OUTDIR/appimagetool" "$APPDIR" "$OUTDIR/PiCrust-v1.0.0-x86_64.AppImage"

echo "==> Linux AppImage created at: $OUTDIR/PiCrust-v1.0.0-x86_64.AppImage"
```

---

## 🤖 Part 3: GitHub Actions Automated Multi-Platform CI/CD

Create `.github/workflows/release-all.yml` to automatically build standalone binaries across macOS, Windows, and Linux on tag creation:

```yaml
name: Release PiCrust

on:
  push:
    tags:
      - 'v*'

jobs:
  macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Build macOS .app Bundle
        run: |
          ./pi-gpui/bundle-macos.sh
          cd pi-gpui/dist-macos
          tar -czf PiCrust-macos-universal.tar.gz "PiCrust.app"
      - uses: softprops/action-gh-release@v2
        with:
          files: pi-gpui/dist-macos/PiCrust-macos-universal.tar.gz

  windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Build Windows Executable
        run: |
          cd pi-gpui
          cargo build --release
          mkdir dist-windows
          copy target\release\pi-gpui.exe dist-windows\PiCrust.exe
          Compress-Archive -Path dist-windows\* -DestinationPath dist-windows\PiCrust-windows-x64.zip
      - uses: softprops/action-gh-release@v2
        with:
          files: pi-gpui/dist-windows/PiCrust-windows-x64.zip

  linux:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install System Dependencies
        run: |
          sudo apt update
          sudo apt install -y libfontconfig1-dev libfreetype6-dev libx11-dev libx11-xcb-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev
      - name: Build Linux AppImage
        run: |
          ./pi-gpui/bundle-linux.sh
      - uses: softprops/action-gh-release@v2
        with:
          files: pi-gpui/dist-linux/PiCrust-*.AppImage
```

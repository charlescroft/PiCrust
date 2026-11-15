# PiCrust Windows Packaging Script
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ScriptDir

# Version comes from Cargo.toml; release assets use it as the filename suffix.
$Version = (Select-String -Path "$ScriptDir\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $Version) { throw "Could not read version from Cargo.toml" }

$OutputDir = Join-Path $ScriptDir "dist-windows\PiCrust"
$ExeName = "PiCrust-windows-x64-$Version.exe"
$ZipName = "PiCrust-windows-x64-$Version.zip"

Write-Host "[picrust] 1. Compiling Release binary (x86_64-pc-windows-msvc) for $Version..." -ForegroundColor Cyan
cargo build --release --target x86_64-pc-windows-msvc

Write-Host "[picrust] 2. Packaging distribution layout..." -ForegroundColor Cyan
if (Test-Path $OutputDir) { Remove-Item -Recurse -Force $OutputDir }
New-Item -ItemType Directory -Path "$ScriptDir\dist-windows" -Force | Out-Null
New-Item -ItemType Directory -Path "$OutputDir\resources\sidecar" -Force | Out-Null

# build.rs embeds assets/icon.ico into the executable (resource id 1), which is
# what the Windows window class and Explorer use for the app icon.
Copy-Item "$ScriptDir\target\x86_64-pc-windows-msvc\release\PiCrust.exe" "$OutputDir\PiCrust.exe"
Copy-Item "$OutputDir\PiCrust.exe" "$ScriptDir\dist-windows\$ExeName" -Force

if (Test-Path "$ScriptDir\dist-sidecar\server.mjs") {
    Copy-Item "$ScriptDir\dist-sidecar\server.mjs" "$OutputDir\resources\sidecar\"
}
if (Test-Path "$ScriptDir\sidecar\api-sidecar.mjs") {
    Copy-Item "$ScriptDir\sidecar\api-sidecar.mjs" "$OutputDir\resources\sidecar\"
    Copy-Item "$ScriptDir\sidecar\router-utils.mjs" "$OutputDir\resources\sidecar\"
}

Write-Host "[picrust] 3. Creating $ZipName..." -ForegroundColor Green
Compress-Archive -Path "$OutputDir\*" -DestinationPath "$ScriptDir\dist-windows\$ZipName" -Force

Write-Host "[picrust] Standalone Windows executable ready: $ScriptDir\dist-windows\$ExeName" -ForegroundColor Green
Write-Host "[picrust] Portable ZIP ready: $ScriptDir\dist-windows\$ZipName" -ForegroundColor Green

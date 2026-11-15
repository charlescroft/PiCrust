# PiCrust Windows Packaging Script
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$OutputDir = Join-Path $ScriptDir "dist-windows\PiCrust"

Write-Host "[picrust] 1. Compiling Release binary (x86_64-pc-windows-msvc)..." -ForegroundColor Cyan
Set-Location $ScriptDir
cargo build --release --target x86_64-pc-windows-msvc

Write-Host "[picrust] 2. Packaging distribution layout..." -ForegroundColor Cyan
if (Test-Path $OutputDir) { Remove-Item -Recurse -Force $OutputDir }
New-Item -ItemType Directory -Path "$ScriptDir\dist-windows" -Force | Out-Null
New-Item -ItemType Directory -Path "$OutputDir\resources\sidecar" -Force | Out-Null

Copy-Item "$ScriptDir\target\x86_64-pc-windows-msvc\release\PiCrust.exe" "$OutputDir\PiCrust.exe"
Copy-Item "$OutputDir\PiCrust.exe" "$ScriptDir\dist-windows\PiCrust.exe" -Force

if (Test-Path "$ScriptDir\dist-sidecar\server.mjs") {
    Copy-Item "$ScriptDir\dist-sidecar\server.mjs" "$OutputDir\resources\sidecar\"
}
if (Test-Path "$ScriptDir\sidecar\api-sidecar.mjs") {
    Copy-Item "$ScriptDir\sidecar\api-sidecar.mjs" "$OutputDir\resources\sidecar\"
    Copy-Item "$ScriptDir\sidecar\router-utils.mjs" "$OutputDir\resources\sidecar\"
}

Write-Host "[picrust] 3. Creating PiCrust-windows-x64.zip..." -ForegroundColor Green
Compress-Archive -Path "$OutputDir\*" -DestinationPath "$ScriptDir\dist-windows\PiCrust-windows-x64.zip" -Force

Write-Host "[picrust] Standalone Windows executable ready: $ScriptDir\dist-windows\PiCrust.exe" -ForegroundColor Green

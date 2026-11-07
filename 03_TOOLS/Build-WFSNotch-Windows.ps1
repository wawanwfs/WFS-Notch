# WFS Notch native Windows build and QA. Run from PowerShell 7 or Windows PowerShell 5.1.
# Requires Rust MSVC, Visual Studio C++ build tools/Windows SDK, WebView2 and Tauri CLI 2.
[CmdletBinding()]
param([switch]$SkipLint)
$ErrorActionPreference = 'Stop'
$App = (Resolve-Path (Join-Path $PSScriptRoot '..\02_APP')).Path
$Backend = Join-Path $App 'src-tauri'
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust/Cargo not found. Install Rust MSVC from https://rustup.rs/ and restart PowerShell.'
}
Write-Host '=== WFS NOTCH - WINDOWS BUILD ===' -ForegroundColor Cyan
cargo --version
Push-Location $Backend
try {
    if (-not $SkipLint) {
        cargo fmt --all
        if ($LASTEXITCODE -ne 0) { throw 'cargo fmt failed' }
        cargo fmt --all --check
        if ($LASTEXITCODE -ne 0) { throw 'cargo fmt check failed' }
        cargo clippy --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'cargo clippy failed' }
    }
    cargo test
    if ($LASTEXITCODE -ne 0) { throw 'cargo test failed' }
} finally { Pop-Location }
Push-Location $App
try {
    cargo tauri build
    if ($LASTEXITCODE -ne 0) { throw 'Tauri build failed. Install CLI: cargo install tauri-cli --version "^2" --locked' }
} finally { Pop-Location }
Write-Host 'Build completed; artifacts:' -ForegroundColor Green
Get-ChildItem (Join-Path $Backend 'target\release\bundle\nsis') -Filter '*.exe' -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host $_.FullName
    Get-FileHash $_.FullName -Algorithm SHA256 | Format-Table -AutoSize
}

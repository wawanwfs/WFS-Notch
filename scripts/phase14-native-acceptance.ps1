# WFS Notch Phase 14 native acceptance inventory (read-only, no admin, no uploads).
# Run on an interactive Windows 10/11 desktop. This script NEVER installs,
# executes or modifies programs, system settings, notification permission or DPI.
[CmdletBinding()]
param(
    [string]$InstallerPath = "",
    [string]$PortablePath = "",
    [string]$OutputDirectory = (Join-Path $env:LOCALAPPDATA 'WFS-Notch\NativeQA')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$report = Join-Path $OutputDirectory 'PHASE14_NATIVE_QA.md'
$system = Get-CimInstance Win32_OperatingSystem
$windowsBuild = $system.Version
$windowsName = $system.Caption
$monitorSummary = @('Monitor inventory unavailable; verify manually')
try {
    Add-Type -AssemblyName System.Windows.Forms
    $monitorSummary = @([System.Windows.Forms.Screen]::AllScreens |
        ForEach-Object { "Primary=$($_.Primary), bounds=$($_.Bounds.X),$($_.Bounds.Y),$($_.Bounds.Width)x$($_.Bounds.Height)" })
} catch {
    # RDP/headless environments or locked-down installations may not expose screens.
    $monitorSummary = @('Monitor inventory unavailable; verify manually')
}
$installerSummary = "Not supplied"
$portableSummary = "Not supplied"
if ($InstallerPath) {
    if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
        throw "Installer file not found"
    }
    $hash = Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256
    $installerSummary = $hash.Hash.ToLowerInvariant()
}
if ($PortablePath) {
    if (-not (Test-Path -LiteralPath $PortablePath -PathType Leaf)) {
        throw "Portable file not found"
    }
    $hash = Get-FileHash -LiteralPath $PortablePath -Algorithm SHA256
    $portableSummary = $hash.Hash.ToLowerInvariant()
}
$lines = @(
    '# WFS Notch Phase 14 — physical Windows acceptance',
    '',
    '**Evidence state:** NOT TESTED until a human completes each scenario on an interactive Windows device.',
    "**Environment:** $windowsName, version $windowsBuild",
    '**Collection:** local only; OS version, display geometry and optional hashes; no notification bodies or private media metadata are collected.',
    "**Installer SHA256 (if provided):** $installerSummary",
    "**Portable SHA256 (if provided):** $portableSummary",
    '',
    '## Read-only display inventory (not a functional test)'
)
$lines += $monitorSummary | ForEach-Object { "- $_" }
$lines += @(
    '',
    '## Manual test checklist — replace NOT TESTED after collecting local proof',
    '',
    '| Test | Status | Notes / sanitized evidence |',
    '| --- | --- | --- |',
    '| Windows 10 installer launch and uninstall/reinstall | NOT TESTED | |',
    '| Windows 11 portable launch independently of installer | NOT TESTED | |',
    '| Toast app permission enabled/disabled | NOT TESTED | |',
    '| Focus Assist / DND and lock screen redaction | NOT TESTED | |',
    '| Reminder after sleep/resume, clock rollback and backup restore | NOT TESTED | |',
    '| Fullscreen vs maximized on selected monitor; no focus steal | NOT TESTED | |',
    '| 1–3 monitors, negative coordinates, hotplug/unplug | NOT TESTED | |',
    '| 100/125/150% per-monitor DPI and resolution switch | NOT TESTED | |',
    '| RDP disconnect and reconnect, layout recovery | NOT TESTED | |',
    '| GSMTC Spotify, browser, Media Player capability changes | NOT TESTED | |',
    '| WebView2 keyboard, reduced motion, contrast and hover | NOT TESTED | |',
    '| DST gaps/overlaps and floating timezone ICS import | NOT TESTED | |',
    '',
    '> This script does not prove installer functionality, device control, toast delivery or WebView2 visual correctness.',
    '> Keep private event titles, screen contents, user paths and session metadata out of shared artifacts.'
)
$lines | Set-Content -LiteralPath $report -Encoding UTF8
Write-Host "Local WFS native QA report created at: $report"
Write-Host 'All interactive manual test statuses remain NOT TESTED.'

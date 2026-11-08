# WFS Notch Phase 16: local, read-only Windows 10/11 manual QA evidence.
# No app launch, permissions, OS changes, private content capture or upload.
[CmdletBinding()]
param(
    [string]$InstallerPath = "",
    [string]$PortablePath = "",
    [string]$OutputDirectory = (Join-Path $env:LOCALAPPDATA 'WFS-Notch\NativeQA'),
    [switch]$Interactive
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-VerifiedFileHash {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) { return 'NOT PROVIDED' }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw 'A supplied installer/portable file does not exist. No code was executed.'
    }
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

$os = Get-CimInstance -ClassName Win32_OperatingSystem
$osName = [string]$os.Caption
$osVersion = [string]$os.Version
$installerHash = Get-VerifiedFileHash -Path $InstallerPath
$portableHash = Get-VerifiedFileHash -Path $PortablePath

$monitors = @('Monitor inventory unavailable: verify physically')
try {
    Add-Type -AssemblyName System.Windows.Forms
    $monitors = @(
        [System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
            'Primary={0}; X={1}; Y={2}; Width={3}; Height={4}' -f $_.Primary, $_.Bounds.X, $_.Bounds.Y, $_.Bounds.Width, $_.Bounds.Height
        }
    )
} catch {
    $monitors = @('Monitor inventory unavailable: verify physically')
}

$scenarios = @(
    'NSIS install/uninstall/upgrade and backup recovery',
    'Portable launch and process lifecycle',
    'Toast opt-in, privacy, DND/Focus Assist and locked screen',
    'Reminder sleep/resume, local clock adjustment and backup restore',
    'Selected monitor 1-3, fullscreen/maximized/borderless',
    'DPI 100/125/150%, resolution, negative origins and hotplug',
    'Remote Desktop disconnect/reconnect and no focus stealing',
    'GSMTC real Spotify, browser and Windows Media Player sessions',
    'Native WebView2 keyboard/screen reader/reduced motion/contrast',
    'Calendar timed DTEND/DURATION, all-day dates, DST and cancellations'
)
$results = New-Object 'System.Collections.Generic.List[string]'
foreach ($scenario in $scenarios) {
    $status = 'NOT TESTED'
    if ($Interactive) {
        Write-Host ''
        Write-Host ('Manual test: ' + $scenario)
        Write-Host 'Perform the test yourself on this interactive Windows device first.'
        $response = (Read-Host 'Record only user-reported PASS, FAIL, or NOT TESTED').Trim().ToUpperInvariant()
        if ($response -eq 'PASS') {
            $status = 'USER-REPORTED PASS (not automatically verified)'
        } elseif ($response -eq 'FAIL') {
            $status = 'USER-REPORTED FAIL'
        }
    }
    $results.Add('| ' + $scenario + ' | ' + $status + ' |')
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$reportFile = Join-Path $OutputDirectory 'PHASE16_NATIVE_QA.md'
$lines = @(
    '# WFS Notch Phase 16: local Windows acceptance record',
    '',
    'Generated locally. No upload or OS modifications. Only OS inventory,'
    'file SHA256 for supplied paths and optional user-entered statuses.',
    'Automated GitHub Windows packaging is not real interactive Windows QA.',
    '',
    ('- OS: ' + $osName),
    ('- Version: ' + $osVersion),
    ('- Installer SHA256: ' + $installerHash),
    ('- Portable SHA256: ' + $portableHash),
    ('- Report created: ' + [DateTimeOffset]::Now.ToString('yyyy-MM-dd HH:mm:ss zzz')),
    '',
    '## Read-only monitor inventory',
    ''
)
foreach ($monitor in $monitors) { $lines += ('- ' + $monitor) }
$lines += @(
    '',
    '## Manually verified acceptance matrix',
    '',
    '| Scenario | Evidence / status |',
    '| --- | --- |'
)
$lines += $results.ToArray()
$lines += @(
    '',
    '## Additional evidence',
    '',
    'Attach only sanitized, personally reviewed local screenshots or notes.',
    'Never attach notification contents, user filenames, secrets or raw media.',
    'This report does not imply any test passed unless manually observed.',
    ''
)
[System.IO.File]::WriteAllLines($reportFile, $lines, [System.Text.UTF8Encoding]::new($false))
Write-Host ('Saved local read-only QA report: ' + $reportFile)

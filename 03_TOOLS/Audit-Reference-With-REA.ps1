<#
.SYNOPSIS
  Analyze a local, authorized Windows EXE with official REA tools.
.DESCRIPTION
  Reads supplied EXE on disk. Never launches the EXE; saves potentially
  sensitive analysis into a folder outside the public Git repository.
  -Deep additionally requests native analysis from an installed Ghidra provider.
.EXAMPLE
  .\03_TOOLS\Audit-Reference-With-REA.ps1 -ReferenceExe "$env:USERPROFILE\Downloads\Poni_1.5.0_portable.exe"
.EXAMPLE
  .\03_TOOLS\Audit-Reference-With-REA.ps1 -ReferenceExe "C:\Samples\Poni_1.5.0_portable.exe" -Deep
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$ReferenceExe,
  [string]$OutputDirectory = "$env:LOCALAPPDATA\WFSNotch\private-rea-audit",
  [switch]$Deep
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$reaCommand = Get-Command 'rea' -ErrorAction SilentlyContinue
if (-not $reaCommand) { throw 'REA is not installed. Run Install-REA-Craft-Windows.ps1 -InstallREA first.' }
$reference = (Resolve-Path -LiteralPath $ReferenceExe -ErrorAction Stop).ProviderPath
if (-not [System.IO.File]::Exists($reference)) { throw "Reference not found: $reference" }
if ([System.IO.Path]::GetExtension($reference) -ne '.exe') { throw 'Expected a Windows .exe input.' }
$hash = (Get-FileHash -LiteralPath $reference -Algorithm SHA256).Hash.ToLowerInvariant()
$sessionName = "audit-" + (Get-Date -Format 'yyyyMMdd-HHmmss')
$session = Join-Path $OutputDirectory $sessionName
New-Item -ItemType Directory -Force -Path $session | Out-Null
$results = New-Object System.Collections.ArrayList

function Invoke-REAReadOnly([string]$Name, [string[]]$Arguments) {
  $stdout = Join-Path $session "$Name.json"
  $stderr = Join-Path $session "$Name.stderr.txt"
  Write-Host "Inspect: $Name"
  & rea @Arguments 1> $stdout 2> $stderr
  $code = $LASTEXITCODE
  [void]$results.Add([ordered]@{ command=$Name; exit_code=$code; output=$stdout; stderr=$stderr })
  if ($code -ne 0) {
    Write-Warning "$Name returned exit code $code. Inspect its stderr; this is not a successful analysis."
  }
}

Write-Host "Reference: $reference"
Write-Host "SHA-256: $hash"
Write-Host 'The sample will NOT be executed.'
Invoke-REAReadOnly 'rust-binary' @('inspect-rust-binary', $reference, '--json')
Invoke-REAReadOnly 'pe-imports' @('inspect-pe-imports', $reference, '--json')
if ($Deep) {
  Write-Warning 'Deep analysis uses the configured native provider. Requires installed Ghidra + compatible JDK on Windows.'
  Invoke-REAReadOnly 'ghidra-analysis' @('analyze', $reference, '--provider', 'ghidra', '--json')
}
$report = [ordered]@{
  reference_file = [System.IO.Path]::GetFileName($reference)
  reference_sha256 = $hash
  generated_at = (Get-Date).ToUniversalTime().ToString('o')
  method = 'Static only: no direct execution of reference EXE'
  deep_analysis_requested = [bool]$Deep
  commands = @($results.ToArray())
  note = 'Keep raw output local. Do not push third-party pseudocode or original binaries into the public repository.'
}
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $session 'audit-index.json') -Encoding UTF8
Write-Host "Evidence: $session"
Write-Host 'Interpret unknown or partial outputs explicitly; passing syntax checks is not evidence of full functional parity.'

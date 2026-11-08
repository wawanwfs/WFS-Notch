<#
.SYNOPSIS
  Install the officially released REA CLI and/or Craft Anything Windows portable package.
.DESCRIPTION
  Runs ONLY when an explicit install switch is supplied. Downloads Craft Anything
  from storytold/craft-anything and verifies its release SHA256SUMS before extraction.
  REA comes from the official npm package "rea-agents". This does NOT execute
  unknown reference binaries, install Ghidra, modify WFS source, or open tunnels.
.EXAMPLE
  .\03_TOOLS\Install-REA-Craft-Windows.ps1 -InstallCraftAnything -InstallREA
#>
[CmdletBinding()]
param(
  [switch]$InstallCraftAnything,
  [switch]$InstallREA,
  [string]$CraftVersion = '0.5.0',
  [string]$Destination = "$env:LOCALAPPDATA\WFSNotch\Tools"
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Test-NodeSupported {
  $nodeCommand = Get-Command 'node' -ErrorAction SilentlyContinue
  $npmCommand = Get-Command 'npm' -ErrorAction SilentlyContinue
  if (-not $nodeCommand -or -not $npmCommand) {
    throw 'Node.js and npm are required for REA. Install a supported version from nodejs.org first.'
  }
  $raw = (& node -p 'process.versions.node').Trim()
  $version = [version]$raw
  $supported = ($version.Major -eq 22 -and $version -ge [version]'22.19.0') -or
    ($version.Major -eq 24 -and $version -ge [version]'24.11.0') -or
    ($version.Major -ge 26)
  if (-not $supported) {
    throw "REA requires Node.js 22.19+, 24.11+, or 26+. Your version: $raw"
  }
  Write-Host "Node.js $raw / npm available"
}

function Install-CraftAnything {
  if ($CraftVersion -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected a numeric Craft Anything release version.' }
  $fileName = "craft-anything-$CraftVersion-windows-x64-portable.zip"
  $urlRoot = "https://github.com/storytold/craft-anything/releases/download/v$CraftVersion"
  $downloadDirectory = Join-Path $Destination 'downloads'
  $archivePath = Join-Path $downloadDirectory $fileName
  $sumsPath = Join-Path $downloadDirectory "SHA256SUMS-v$CraftVersion.txt"
  $installDirectory = Join-Path $Destination "craft-anything-$CraftVersion"
  New-Item -ItemType Directory -Force -Path $downloadDirectory | Out-Null

  Write-Host 'Downloading official release manifest and portable archive…'
  Invoke-WebRequest -Uri "$urlRoot/SHA256SUMS.txt" -OutFile $sumsPath -UseBasicParsing
  Invoke-WebRequest -Uri "$urlRoot/$fileName" -OutFile $archivePath -UseBasicParsing

  $pattern = '^(?<hash>[0-9a-fA-F]{64})\s+\*?' + [regex]::Escape($fileName) + '$'
  $expected = $null
  foreach ($line in (Get-Content -LiteralPath $sumsPath)) {
    $match = [regex]::Match($line.Trim(), $pattern)
    if ($match.Success) { $expected = $match.Groups['hash'].Value.ToLowerInvariant(); break }
  }
  if (-not $expected) { throw "Release checksums did not include $fileName" }
  $actual = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -ne $expected) {
    Remove-Item -LiteralPath $archivePath -Force
    throw 'SHA-256 verification FAILED. Archive was deleted. Do not run it.'
  }
  Write-Host "Craft Anything SHA-256 VERIFIED: $actual"
  if (Test-Path -LiteralPath $installDirectory) {
    Write-Host "The directory already exists: $installDirectory (not overwritten)"
  } else {
    Expand-Archive -LiteralPath $archivePath -DestinationPath $installDirectory
    Write-Host "Craft Anything extracted to: $installDirectory"
  }
  Write-Warning 'Checksum confirms the manifest matches the archive; it is not a signature verification. Only execute packages you trust.'
}

Write-Host '=== WFS Notch · REA + Craft Anything Toolchain ==='
Write-Host "Official REA: https://github.com/morluto/rea"
Write-Host "Official Craft Anything: https://github.com/storytold/craft-anything"
if (-not $InstallREA -and -not $InstallCraftAnything) {
  Write-Host 'No install switch provided. Nothing changed.'
  Write-Host 'Example: .\03_TOOLS\Install-REA-Craft-Windows.ps1 -InstallREA -InstallCraftAnything'
  exit 0
}
if ($InstallREA) {
  Test-NodeSupported
  Write-Host 'Installing official npm package rea-agents. npm install may execute dependency scripts.'
  & npm install --global rea-agents
  if ($LASTEXITCODE -ne 0) { throw "npm install failed with code $LASTEXITCODE" }
  & rea doctor --json
  if ($LASTEXITCODE -ne 0) {
    Write-Warning 'rea doctor reported missing optional providers or agent registration. See docs.'
  }
  Write-Host 'To add MCP/skills to Codex or Claude after reviewing the plan, run: rea setup'
}
if ($InstallCraftAnything) {
  Install-CraftAnything
}
Write-Host 'Completed requested setup steps. Ghidra, REA agent registration and real Windows app comparisons remain separate opt-in steps.'

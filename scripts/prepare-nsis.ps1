#Requires -Version 7.0
<#
.SYNOPSIS
    Cache official verified NSIS toolkit for Tauri Windows packaging.
.DESCRIPTION
    Avoid transient GitHub HTTP 504 by retrying only Tauri's official GitHub
    release assets. Check exact upstream tauri-bundler pinned SHA1 values.
    SHA1 matches Tauri's integrity pins, not an independent code signature.
#>
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$cacheRoot = Join-Path $env:LOCALAPPDATA 'tauri'
$nsisRoot = Join-Path $cacheRoot 'NSIS'
$toolHash = 'DB14F8AF2EA346B786C6AE19343F61AAF5E09B8E'
$binaryHash = '6D45D4E8CCF18FE1FDAC9164058F356102A2ECF2'
$pluginHash = '75197FEE3C6A814FE035788D1C34EAD39349B860'
$pluginRelative = 'Plugins/x86-unicode/additional/nsis_tauri_utils.dll'
$required = @('NSIS.exe','makensis.exe','Bin/makensis.exe','Include/MUI2.nsh','Include/FileFunc.nsh',
  'Include/x64.nsh','Include/nsDialogs.nsh','Include/WinMessages.nsh','Include/Win/COM.nsh',
  'Include/Win/Propkey.nsh','Include/Win/RestartManager.nsh',
  'Stubs/lzma-x86-unicode','Stubs/lzma_solid-x86-unicode',$pluginRelative)

function Test-CachedToolkit {
    foreach ($relative in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $nsisRoot $relative) -PathType Leaf)) { return $false }
    }
    $exeHash = (Get-FileHash -LiteralPath (Join-Path $nsisRoot 'NSIS.exe') -Algorithm SHA1).Hash
    $dllHash = (Get-FileHash -LiteralPath (Join-Path $nsisRoot $pluginRelative) -Algorithm SHA1).Hash
    return ($exeHash -eq $binaryHash -and $dllHash -eq $pluginHash)
}

function Receive-OfficialAsset {
    param([string]$FileName,[string]$BrowserUrl,[string]$AssetApi,[string]$ExpectedSha1,[string]$Destination)
    for ($attempt = 1; $attempt -le 8; $attempt++) {
        $apiMode = ($attempt % 2) -eq 0
        $uri = if ($apiMode) { $AssetApi } else { $BrowserUrl }
        $headers = @{ 'User-Agent' = 'WFS-Notch-CI'; 'Accept' = 'application/octet-stream' }
        if ($apiMode -and -not [string]::IsNullOrEmpty($env:GITHUB_TOKEN)) {
            $headers['Authorization'] = "Bearer $env:GITHUB_TOKEN"
            $headers['X-GitHub-Api-Version'] = '2022-11-28'
        }
        try {
            if (Test-Path -LiteralPath $Destination) { Remove-Item -LiteralPath $Destination -Force }
            $endpoint = if ($apiMode) { 'GitHub API' } else { 'GitHub Releases' }
            Write-Host "Fetching official $FileName (attempt $attempt of 8; $endpoint)"
            Invoke-WebRequest -Uri $uri -Headers $headers -OutFile $Destination -TimeoutSec 90 -MaximumRedirection 10
            $actual = (Get-FileHash -LiteralPath $Destination -Algorithm SHA1).Hash
            if ($actual -ne $ExpectedSha1) { throw "$FileName failed Tauri's pinned SHA1 validation" }
            Write-Host "$FileName verified against official Tauri SHA1"
            return
        } catch {
            Write-Warning "Official download attempt $attempt for $FileName failed: $($_.Exception.Message)"
            if (Test-Path -LiteralPath $Destination) { Remove-Item -LiteralPath $Destination -Force }
            if ($attempt -eq 8) { throw "Unable to download official $FileName; refusing unverified packaging." }
            Start-Sleep -Seconds ([Math]::Min(25, 2 * $attempt))
        }
    }
}

if (Test-CachedToolkit) {
    Write-Host 'Verified NSIS toolkit already cached.'
    exit 0
}
if (Test-Path -LiteralPath $nsisRoot) { Remove-Item -LiteralPath $nsisRoot -Recurse -Force }
New-Item -Path $cacheRoot -ItemType Directory -Force | Out-Null
$work = Join-Path ([IO.Path]::GetTempPath()) ("wfs-nsis-" + [guid]::NewGuid().ToString('N'))
New-Item -Path $work -ItemType Directory -Force | Out-Null
try {
    $archive = Join-Path $work 'nsis-3.13.zip'
    Receive-OfficialAsset -FileName 'nsis-3.13.zip' -BrowserUrl 'https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip' -AssetApi 'https://api.github.com/repos/tauri-apps/binary-releases/releases/assets/623832696' -ExpectedSha1 $toolHash -Destination $archive
    Expand-Archive -LiteralPath $archive -DestinationPath $work -Force
    $extracted = Join-Path $work 'nsis-3.13'
    if (-not (Test-Path -LiteralPath (Join-Path $extracted 'NSIS.exe') -PathType Leaf)) {
        throw 'Official archive lacks expected nsis-3.13/NSIS.exe'
    }
    Move-Item -LiteralPath $extracted -Destination $nsisRoot
    $pluginPath = Join-Path $nsisRoot $pluginRelative
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $pluginPath) | Out-Null
    Receive-OfficialAsset -FileName 'nsis_tauri_utils.dll' -BrowserUrl 'https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll' -AssetApi 'https://api.github.com/repos/tauri-apps/nsis-tauri-utils/releases/assets/345882097' -ExpectedSha1 $pluginHash -Destination $pluginPath
    if (-not (Test-CachedToolkit)) { throw 'NSIS toolkit missing required files or hashes' }
    Write-Host 'Verified NSIS toolkit prepared in the Tauri per-user cache.'
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}

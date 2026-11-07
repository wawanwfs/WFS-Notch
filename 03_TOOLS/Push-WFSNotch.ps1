# Publish changes under the configured owner identity, without force-push.
# Every push MUST be authenticated to @wawanwfs; no CI bot pushes.
[CmdletBinding()]
param([string]$Message = 'chore: update WFS Notch')
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$expectedEmail = '90094394+wawanwfs@users.noreply.github.com'
$expectedName = 'WAHYU FAJAR SETIAWAN'
Push-Location $Root
try {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw 'GitHub CLI required; install from https://cli.github.com/' }
    $login = (& gh api user --jq '.login').Trim()
    if ($LASTEXITCODE -ne 0 -or $login -ne 'wawanwfs') { throw 'Must authenticate GitHub CLI as wawanwfs.' }
    & git config --local user.name $expectedName
    & git config --local user.email $expectedEmail
    & git config --local user.useConfigOnly true
    & git add --all
    if ($LASTEXITCODE -ne 0) { throw 'git add failed' }
    & git diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        & git commit -m $Message
        if ($LASTEXITCODE -ne 0) { throw 'Commit failed' }
    }
    $origin = (& git remote get-url origin).Trim()
    if ($origin -notmatch '^((https://github\.com/)|(git@github\.com:))wawanwfs/WFS-Notch(\.git)?$') { throw "Unexpected remote: $origin" }
    & git fetch origin main --quiet
    if ($LASTEXITCODE -ne 0) { throw 'Fetch failed' }
    # Prevent pushing any outgoing commit whose author identity doesn't match the owner's configured noreply address.
    $outgoing = @(& git rev-list 'origin/main..HEAD')
    if ($LASTEXITCODE -ne 0) { throw 'Unable to list outgoing commits' }
    foreach ($hash in $outgoing) {
        $email = (& git show -s --format='%ae' $hash).Trim()
        if ($email -ne $expectedEmail) { throw "Commit $hash has a different author email: $email" }
    }
    & git push origin HEAD:main
    if ($LASTEXITCODE -ne 0) { throw 'Push failed. No force push attempted.' }
    Write-Host 'Push successful on behalf of @wawanwfs. Check GitHub Actions for results.' -ForegroundColor Green
} finally { Pop-Location }

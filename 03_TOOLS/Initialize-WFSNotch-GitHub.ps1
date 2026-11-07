# Local bootstrap only. Does not rewrite someone else's Git history.
# Prerequisites: Git for Windows; GitHub CLI (gh) authenticated as wawanwfs.
# Run: .\03_TOOLS\Initialize-WFSNotch-GitHub.ps1
[CmdletBinding()]
param(
    [string]$Repository = 'wawanwfs/WFS-Notch',
    [switch]$Public,
    [switch]$NoPush
)
$ErrorActionPreference = 'Stop'
$ExpectedOwner = 'wawanwfs'
$ExpectedName = 'WAHYU FAJAR SETIAWAN'
$ExpectedEmail = '90094394+wawanwfs@users.noreply.github.com'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ($Repository -ne 'wawanwfs/WFS-Notch') { throw 'This script is intentionally restricted to wawanwfs/WFS-Notch.' }
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw 'Install Git for Windows first: https://git-scm.com/downloads/win' }
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw 'Install GitHub CLI: https://cli.github.com/ then run gh auth login.' }
$ghLogin = (& gh api user --jq '.login').Trim()
if ($LASTEXITCODE -ne 0 -or $ghLogin -ne $ExpectedOwner) {
    throw "Authenticated GitHub identity must be $ExpectedOwner; got '$ghLogin'. Run gh auth login or gh auth switch."
}
Push-Location $Root
try {
    if (-not (Test-Path '.git')) {
        & git init -b main
        if ($LASTEXITCODE -ne 0) { throw 'git init failed' }
    }
    & git config --local user.name $ExpectedName
    & git config --local user.email $ExpectedEmail
    & git config --local user.useConfigOnly true
    & git config --local --get user.name
    & git config --local --get user.email
    & gh repo view $Repository --json name --jq '.name' 2>$null
    $alreadyExists = $LASTEXITCODE -eq 0
    if (-not $alreadyExists) {
        $visibility = if ($Public) { '--public' } else { '--private' }
        & gh repo create $Repository $visibility --source . --remote origin
        if ($LASTEXITCODE -ne 0) { throw 'Repository creation failed; check GitHub CLI permissions.' }
    } else {
        $expectedUrl = "https://github.com/$Repository.git"
        $origin = (& git remote get-url origin 2>$null)
        if ($LASTEXITCODE -ne 0) {
            & git remote add origin $expectedUrl
            if ($LASTEXITCODE -ne 0) { throw 'Could not add origin remote' }
        } elseif ($origin -notmatch '^((https://github\.com/)|(git@github\.com:))wawanwfs/WFS-Notch(\.git)?$') {
            throw "Existing origin points elsewhere: $origin. Resolve manually before publishing."
        }
    }
    & git add --all
    if ($LASTEXITCODE -ne 0) { throw 'git add failed' }
    & git diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        & git commit -m 'chore: initialize WFS Notch and Windows CI'
        if ($LASTEXITCODE -ne 0) { throw 'git commit failed' }
    }
    if (-not $NoPush) {
        $identity = (& git log -1 --format='%an <%ae>').Trim()
        if ($identity -ne "$ExpectedName <$ExpectedEmail>") { throw "HEAD is not attributed to the expected author: $identity" }
        & git push -u origin HEAD:main
        if ($LASTEXITCODE -ne 0) { throw 'Push failed; verify branch and permissions' }
        Write-Host "Published https://github.com/$Repository" -ForegroundColor Green
    } else {
        Write-Host 'Repository created locally but push skipped (-NoPush)' -ForegroundColor Yellow
    }
} finally { Pop-Location }

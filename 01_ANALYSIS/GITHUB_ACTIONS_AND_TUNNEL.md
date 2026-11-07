# GitHub Actions audit, trust boundaries, and remote access

## Continuous integration

- `.github/workflows/windows-build.yml` triggers on `push` to `main`, PR against `main`, and manual workflow dispatch.
- `attribution-audit` checks that pushes to `main` came from the authenticated `wawanwfs` actor, and flags commit author/committer metadata that differs from the configured name and GitHub noreply email. This is post-push auditing, not an access-control rule.
- `browser-audit` runs JavaScript syntax, static audit, and Playwright regression/integration on Ubuntu. Browser tests use a local mock and are NOT native Windows verification.
- `rust-quality` separately runs `cargo fmt --check`, native Rust unit tests and clippy (`-D warnings`), while independent `windows-build` builds an unsigned NSIS installer on a GitHub-hosted Windows x64 runner using `tauri-apps/tauri-action@v1`.
- Build artifacts and checksums are retained in GitHub Actions. A successful build proves compilation and packaging, not all app functions or Windows-specific runtime integration.
- Native UI automation, Windows Notifications, media APIs and telemetry require separate integration tests on a real/interactive Windows desktop.
- The workflow uses `permissions: contents: read`, does not persist credentials, and never commits or pushes files automatically.

## Identity and attribution

- Repository owner: `wawanwfs` (verified via authenticated GitHub connector).
- Local commit name: `WAHYU FAJAR SETIAWAN`.
- Local commit email: `90094394+wawanwfs@users.noreply.github.com`.
- Bootstrap/publish PowerShell scripts verify `gh api user` returns `wawanwfs` before acting.
- Authenticated pusher, Git commit author and committer are distinct metadata. Commit author email is not cryptographic proof of personhood. Enable repository rulesets and require signed commits if verified provenance is mandatory; these policies cannot be set by the scripts automatically.

## Cloudflare Tunnel decision

Do NOT expose a GitHub-hosted runner over an unrestricted public Cloudflare Tunnel for full RDP/remote-shell control. GitHub-hosted Windows runners are ephemeral and not a persistent interactive workstation. Opening a tunnel could expose credentials, source code and pipeline environment, especially for pull_request jobs.

Safer default: GitHub Actions logs, test reports, Playwright screenshots, and compiled artifact downloads. If interactive control is later needed, use a dedicated self-hosted Windows test VM (not a developer's daily machine) with time-limited Cloudflare Access, identity enforcement/MFA, least privilege, no embedded repo token, separate audit logging, and a hard kill switch. Configure it outside PR workflows, and require explicit approval before each live session. This package intentionally contains no tunnel token, install step, or remotely exposed runner.

## Publication state

As prepared, repository `wawanwfs/WFS-Notch` does not yet exist and creation is not supported by the current GitHub connector actions. CI has not run on GitHub. Use `03_TOOLS/Initialize-WFSNotch-GitHub.ps1` from a GitHub CLI session authenticated as `wawanwfs`, or create an empty private repository named `WFS-Notch` under `wawanwfs` in the GitHub UI, then publish with the script.

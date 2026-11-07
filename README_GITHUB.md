# WFS Notch — GitHub-ready CI package

This is the full WFS Notch Phase 3 codebase plus GitHub Actions and owner-controlled publishing scripts. It is **not** proof of a passing GitHub-hosted native Windows build until the new repository exists and the workflow actually runs.

## Owner identity
- GitHub repository: `wawanwfs/WFS-Notch`.
- Every local publish script checks that `gh` is logged in as `wawanwfs`.
- An `attribution-audit` workflow job flags pushes by another actor and mismatched commit metadata (after the push; it cannot prevent it).
- Each new local commit uses `WAHYU FAJAR SETIAWAN <90094394+wawanwfs@users.noreply.github.com>`.
- Workflows are read-only; they never auto-push, impersonate the owner or expose remote shell/tunnels.

## One-time publishing on Windows 10/11
1. Install [Git for Windows](https://git-scm.com/downloads/win) and [GitHub CLI](https://cli.github.com/) if absent (winget is NOT required).
2. Extract this ZIP anywhere on your PC.
3. In PowerShell in its root folder:

   ```powershell
   gh auth login
   .\03_TOOLS\Initialize-WFSNotch-GitHub.ps1
   ```

   Defaults to a **private** repository; use `-Public` only when you consciously want to publish source publicly. The script checks your GitHub login, initializes git, creates the repo if needed, commits and pushes to `main` with your Git author attribution. This requires appropriate GitHub permissions.

4. Future commits/pushes:

   ```powershell
   .\03_TOOLS\Push-WFSNotch.ps1 -Message 'feat: implement media controls'
   ```

5. After first push, open `https://github.com/wawanwfs/WFS-Notch/actions` to inspect `WFS Notch — Windows CI & Browser Audit`. Check logs, screenshot artifacts, and NSIS installer artifacts. Build failures must be fixed before the application can be considered a working native Windows release.

If GitHub CLI is unavailable but you can access GitHub in a browser, create an empty repo at `https://github.com/new` with the exact name `WFS-Notch`, **without README/gitignore auto-initialization**. After installing `gh` and signing in, run the same PowerShell script.

## What is and isn't tested
- Current local tests in this environment: JavaScript syntax, structural audit, Chromium preview integration/regression.
- GitHub workflow *will attempt*: actual Windows MSVC Rust compile, unit tests, Clippy, Tauri NSIS packaging, hashes, artifact uploads.
- Not yet proven: native Windows UI interaction, true memory/CPU telemetry, silent autostart and tray, OS-level notification and media controls.

Consult [GitHub Actions security and Cloudflare Tunnel policy](01_ANALYSIS/GITHUB_ACTIONS_AND_TUNNEL.md).

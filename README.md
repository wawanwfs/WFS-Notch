# WFS Notch

**An original, local-first Windows desktop command center built with Rust and Tauri 2.** A compact floating notch expands into tasks, notes, focus, calendar, media, and system controls.

![WFS Notch logo](assets/branding/wfs-notch-wordmark.png)

> Version 0.10.0 (development). The Windows installer is unsigned. Native media, multi-monitor and fullscreen behavior still require actual device validation. Feature parity with any reference program is not claimed.

## Features

- Compact/expanded top-edge notch, pin/open state, tray, startup, saved monitor position and accent colors.
- Tasks, local notes with search/autosave, monthly calendar with UTC ICS import/export, habits, focus timer, stopwatch and countdown.
- Windows media title/artist/album, transport keys, local-only disk/network/CPU/RAM inventory.
- Keyboard navigation, reduced-motion preferences, original dark/champagne style.
- Local JSON storage with backup/restore; no account or paid API required.

## Source tree

```text
src/                        # HTML/CSS/JavaScript app and SVG assets
src-tauri/                  # Rust backend, Tauri 2 config and Windows icons
  src/                      # Native commands/modules
  capabilities/             # IPC security
tests/                      # Browser regressions, static audit and identity checks
docs/                       # Feature coverage and roadmap
assets/branding/            # Original WFS logo assets
scripts/build-windows.ps1   # Build/check helper
.github/workflows/ci.yml    # Rust + browser + Windows NSIS Actions
```

Previous phase-numbered folders and research handoff documents have been removed from the active source tree. Git history still retains earlier commits. No third-party binaries or proprietary decompilation outputs are shipped.

## Build on Windows 10/11

Prerequisites: Rust MSVC, Visual Studio C++ Build Tools and Windows SDK, Microsoft Edge WebView2 Runtime, Tauri CLI v2. **No winget is needed.**

```powershell
git clone https://github.com/wawanwfs/WFS-Notch.git
cd WFS-Notch
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
.\scripts\build-windows.ps1
```

Open `src/index.html` in a browser for a **frontend preview**, which does not fabricate native Windows hardware/media signals.

## Security, QA and status

CI runs JavaScript checks, browser regressions, Rust unit tests, Clippy, rustfmt, and unsigned Windows x64 NSIS packaging. A successful build is not proof of real Windows interaction or pixel-perfect reference behavior. Notes/data remain local; hardware inventory is read-only. Privacy-sensitive native notifications are disabled by default.

[Feature coverage](docs/feature-coverage.md) · [Roadmap](docs/roadmap.md)

## Licensing

No open-source redistribution license has been selected. All rights reserved by the repository owner until a license is added.

# WFS Notch

**A private, local-first desktop companion for Windows 10/11 — built with Rust and Tauri 2.**

![WFS Notch](assets/branding/wfs-notch-wordmark.png)

WFS Notch brings an expandable top-edge panel with a calendar, tasks, notes, focus timers, native Windows media controls, device information and optional private desktop reminders. It does not require an account.

## Features

- Lightweight always-on-top notch with selected-monitor placement, DPI-aware centering, tray and autostart.
- Task and note tools, calendar with iCalendar UTC import/export, wellness and focus timers.
- Windows GSMTC media sessions: player selection, playback controls, artwork when supplied by the player.
- Native opt-in calendar notifications. WFS Notch never reads other applications' notifications. Event titles stay private unless enabled; WFS-only Pause Reminders is available.
- Read-only local disk/network/CPU/RAM inventory, theme colors and accessible keyboard navigation.
- Local JSON with backup and restore.

## Project layout

```text
src/                       HTML entry, app state and asset links
  features/                Calendar, media, wellness, system and display UI
  styles/                  Base layout and feature styles
  assets/                  Frontend SVG
src-tauri/                 Rust 2021 / Tauri 2 native Windows application
  src/                     Native modules and commands
  capabilities/            IPC permissions
  icons/                   Windows app icons
tests/                     Browser regressions and static source checks
assets/branding/           Original WFS Notch identity
docs/                      Architecture, security, roadmap, coverage
scripts/                   Build helper
.github/workflows/ci.yml   Windows build, Rust tests and browser CI
```

## Develop and build on Windows

Install Rust (MSVC toolchain), C++ Build Tools and Windows SDK, WebView2 Runtime, and Tauri CLI 2. No `winget` dependency.

```powershell
git clone https://github.com/wawanwfs/WFS-Notch.git
cd WFS-Notch
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
.\scripts\build-windows.ps1
```

For a static browser preview, open `src/index.html`. Browser mode is not an emulator and does **not** fabricate native Windows media, monitors, notifications or device data.

## Security and product maturity

Native notifications are off by default, and their titles are hidden until the user opts in. The notification pause option affects only WFS Notch. All user data remains on the device except features where the user explicitly requests third-party weather or opens an external link. No public tunnel or remote shell is deployed.

Windows CI builds and regression tests verify code quality and package creation, not real-device behavior or complete equivalence to any other product. This is a development release, unsigned and not independently certified on physical Windows machines.

Read [Architecture](docs/architecture.md), [Security](docs/security.md), [Feature coverage](docs/feature-coverage.md), and [Roadmap](docs/roadmap.md).

## License

No open-source license has been granted yet. Rights are reserved by the repository owner until a license is published.

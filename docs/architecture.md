# Architecture

WFS Notch is a **Tauri 2 Windows desktop application** with a static HTML/CSS/JS frontend in `src/` and a Rust backend in `src-tauri/`. There is no cloud control plane.

## Frontend

`src/index.html` is the entry point. `src/app.js` manages state, secure IPC calls and local browser preview fallbacks. The `src/features/` scripts implement calendar, media, system status, display placement, wellness and accessibility; `src/styles/` contains presentation layers. Browser preview never simulates real device state.

## Native application

`src-tauri/src/main.rs` owns Tauri initialization and local application state. Domain modules provide calendar/ICS, notes/tasks, wellness, Windows media sessions, notifications, display positioning and read-only system inventory. `notifications.rs` runs an independent 20-second Rust worker while the process is alive; it reads only the local calendar and sends opt-in reminders through the Tauri notification plugin, even when the WebView is hidden. Tauri command handlers are registered centrally; the client does not directly access OS credentials or the filesystem.

Local state is written to app-specific storage using a backup and temporary file; migrations use Serde defaults for older settings. No third-party binary code or extracted proprietary assets are part of the application.

## Verification

GitHub Actions runs browser-based regression tests, static HTML/Tauri command audits, Rust unit tests, Clippy, rustfmt and NSIS packaging. Physical Windows tests (fullscreen, 1–3 monitors, mixed DPI, notifications, multi-session media) remain required before claiming production readiness.

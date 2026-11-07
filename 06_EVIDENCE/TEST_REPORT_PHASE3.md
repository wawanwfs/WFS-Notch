# Phase 3 test evidence — 2026-11-07

## Checks executed in local container

| Check | Observed result |
|---|---|
| `node --check 02_APP/ui/app.js` | PASS |
| `python 04_TESTS/static_audit.py` | PASS: 57 unique HTML IDs; 22 Rust Tauri commands; JSON, SVG and icon references checked |
| `python 04_TESTS/browser_smoke.py` | PASS: prior Tasks, Notes, timer, Settings regression |
| `python 04_TESTS/browser_phase3.py` | PASS: add, edit and delete events; in-app reminder; create and restore backup; no fake system metrics in preview |
| Browser JavaScript uncaught exceptions | 0 during Phase 3 tests |
| Cargo tests / Clippy | NOT RUN: Rust/Cargo unavailable |
| Windows UI/native telemetry | NOT RUN: Windows runtime unavailable |
| Installer NSIS / signing | NOT RUN |

These are functional browser-preview tests with a localStorage adapter, and don't establish native storage or Windows IPC correctness. The GitHub Actions Windows build recipe is included but not executed here.

## Screenshots
- `browser-calendar-phase3.png` — new Calendar panel with a saved event.
- `browser-system-phase3.png` — System panel in browser-preview mode, intentionally showing no CPU/RAM data.
- `browser-home.png`, `browser-notes.png` — Phase 2 regression screenshots generated with Phase 3 source.

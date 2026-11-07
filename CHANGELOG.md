## 0.5.0 — Phase 5 REA

- Added Rust-persisted water/break/calm counters and an independent, suspend-aware, adjustable countdown.
- Added opt-in weather via Open-Meteo, Day Progress and Notes flashcards.
- Added browser regression tests, expanded navigation and detailed gap matrix against Poni 1.5.0.
- Clearly marked unimplemented native Windows integrations and 100% parity as **not yet achieved**.

# Changelog

## 0.2.0 — Phase 2 (2026-11-07)
- Brand refresh: WFS Notch metallic champagne-gold W mark, wordmark and Windows icons.
- Notes: serialized mutations, autosave tracking and guarded navigation.
- Timer: persistent Rust focus state with pause/resume and elapsed-time recovery.
- Settings: persisted hover/pin preferences, explicit desktop Exit action.
- Storage: backward-compatible JSON, backup recovery and corruption preservation.
- Accessibility/layout improvements and smoke test suite.
- Source-only deliverable: **not yet compiled as a Windows EXE**.

## 0.1.0 — Phase 1
- Original Tauri 2 / Rust starter, basic window shell, Tasks, Notes, Focus and accent settings.

## 0.3.0 / Phase 3 — continued development
- Introduced validated local calendar events with CRUD, timezone-aware display, explicit per-event in-app reminders.
- Added in-app reminder banners. They work only while the app is running and visible; no native Windows notification integration.
- Added `get_system_snapshot` Rust command using pinned `sysinfo = 0.33.1` for CPU and memory; intentionally shows unavailable data in browser preview.
- Added user-controlled local backup creation, listing and restore with confirmation and identifier validation.
- Extended overview, seven-module responsive tabs, system UI and backup settings.
- Added browser Phase 3 tests and regression/static audit results.
- Windows-native Rust compilation and NSIS installer remain unverified and unreleased.

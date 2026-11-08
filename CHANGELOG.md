## 0.8.0 — Native Windows media session metadata (validation pending)

- Added read-only Windows GSMTC current-session title, artist, album, playback status, track position and duration in Rust.
- Added native-only now-playing panel and media timeline; browser preview does not invent track titles.
- Polling only occurs while the Media panel is active, with an explicit Refresh control.
- Album art, synced lyrics, playlist management, multi-player selection and interactive device runtime QA remain outstanding.

## 0.7.0 — Native disk and network monitoring (CI validation required)

- Added Rust read-only per-volume capacity/used/available bytes via sysinfo.
- Added Rust local network interface received/transmitted byte totals, without Wi-Fi scanning, MAC/IP collection or API calls.
- Added on-demand and panel-scoped polling with 15-second cache and a responsive System view.
- Browser preview intentionally displays no fabricated system readings; Rust unit/static/browser tests extended.
- Native hardware telemetry depends on device/driver support; not equivalent to Poni's GPU/temperature/control panel.

## 0.6.0 — Local calendar interoperability and Notes search (under validation)

- Added UTC iCalendar import/export as Rust commands with transactional state updates, UID deduplication, strict validation and unit tests.
- Added responsive monthly calendar grid with event badges and keyboard-accessible day selection.
- Added case-insensitive full-text search of saved note titles and bodies.
- Added Chromium regressions and CI coverage. Native Windows interactive validation still required.
- Scope intentionally excludes automatic sync with Google/Outlook, native file dialog, TZID/floating dates and 100% Poni feature parity.

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

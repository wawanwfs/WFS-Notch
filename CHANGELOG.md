## 0.11.0 — Native calendar reminder scheduling (development)

- Moved scheduling of WFS-owned Windows calendar reminder to a Rust worker that keeps running while the notch UI is hidden or collapsed (application must remain running).
- Unified scheduled and manual reminders behind the same privacy checks and in-memory per-occurrence deduplication.
- Added status command showing upcoming reminder count and pause/consent status without revealing event titles; browser preview does not fabricate a native scheduler.
- Added Rust tests for late delivery windows, rescheduling, pause/opt-out, reminder redaction and expiration.
- Kept existing opted-in notifications, media controls, monitor placement and WFS branding intact.
- Remaining: real Windows toast permission/delivery tests, durable ledger across app restart, Windows fullscreen/hot-plug QA and more complete reference feature coverage.

## 0.10.1 — Product layout and private notification pause (development)

- Rebuilt the source tree as a conventional Rust/Tauri application, with domain-level feature and style modules.
- Added a local WFS-only Pause Reminders setting; native notifications are opt-in and redacted by default.
- Updated regression fixtures, Rust module imports, documentation and CI references after file renames.
- Does not enable third-party notification scraping, alter global Do Not Disturb, or establish native Windows runtime parity.

# Changelog

## 0.10.0 (development)
- Replaced phase-numbered/research-bundle repository layout with standard Rust/Tauri project folders.
- Retained all existing application source and original WFS branding.
- Added opt-in WFS native calendar toast reminders, private by default; Windows notification reading is not implemented.
- Added explicit Windows GSMTC player session selection and controls, bounded PNG/JPEG/GIF cover artwork and interpolated playback progress.
- Added manual always-on-top toggle for presentations/fullscreen games; automatic fullscreen detection remains pending.
- All new native Windows functionality still requires physical device QA; do not infer 100% parity from a CI build.

## 0.9.0
- DPI-aware monitor preference and persistent notch placement.

## 0.8.0
- WinRT media metadata, keyboard navigation, reduced-motion and dark/champagne UI improvements.

## 0.7.0
- Native read-only disk/network information.

## 0.6.0
- Month calendar, UTC ICS import/export and Notes search.

## Earlier releases
- Tasks, notes, focus, habits, UI shell, tray, backups and timers.

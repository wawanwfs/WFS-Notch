## 0.13.0 — Deterministic notification recovery & fullscreen QA (development)

- Refactored WFS reminder dispatch into a production-used time-injected sender seam to deterministically exercise real Rust state transactions, restart, clock changes, edited/cancelled reminders, opt-out and notification-API failure retry without invoking Windows on test runners.
- Retain delivery reservations for a bounded 30-day period, separating deduplication history from the five-minute reminder delivery grace; this improves backward clock-jump resilience but cannot provide unbounded or exactly-once guarantees.
- Preserve the current reminder journal across local backup restore to prevent replaying already-enqueued calendar notifications after restoring older state.
- Avoid mistakenly classifying a window stretched across multiple monitors as an exclusive fullscreen window; expand pure geometry regression for negative coordinates and 100/125/150% DPI.
- Keep original WFS dark/champagne-gold design, application structure and privacy defaults; licensed lyrics, golden-image reference baselines and physical Windows 10/11 interactive QA remain outstanding.
- 38-category weighting remains 3 Implemented, 16 Partial, 19 Gap = 28.9%.

## 0.12.0 — Windows reliability and native media refinement (development)

- Added an at-most-once persistent WFS reminder reservation ledger in the existing Rust local state file, bounded sleep/resume catch-up, edited-event keys, failure retry and deterministic tests. Windows toast enqueue is not an end-user display receipt.
- Added WinRT per-app toast setting inspection in Settings without conflating it with Tauri desktop permission or Focus Assist.
- Added opt-in Win32 same-monitor fullscreen detection and background monitor DPI/topology re-positioning; native hardware QA remains open.
- Added selected GSMTC player metadata/progress/artwork and capability-aware transport controls with disconnect handling.
- Added compact/expanded transitions, scrollable small viewport UI and 64 browser screenshot checks across 16 modules and three simulated DPR values.
- Category-weighted progress remains 28.9% (3 Implemented, 16 Partial, 19 Gap); see Phase 12 report.
- Windows installer is unsigned; do not interpret CI compilation as real-device behavioral validation.

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

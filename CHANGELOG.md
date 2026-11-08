## 0.23.0 — Guarded one-time local series day shift (Batch Workstream A)

- New Tauri preview/apply commands let users move **existing** imported recurrence occurrences by a whole-day offset (-365..-1 or +1..+365), with a preview count, old/new bounds, reminder count, exact-start conflicts and an expiring-by-state freshness token; explicit user confirmation required.
- Only source-backed, individually unedited true all-day DATE or explicitly UTC timed recurrence instances qualify. IANA TZID/floating times, previously edited instances, canceled whole series, invalid bounds and unsupported recurrence overrides fail closed to avoid DST or data-loss surprises.
- Shift is transactional across all existing local instances, preserves event IDs, titles, descriptions, exclusive ends, original cancellation markers and reminder ledger history. The original RFC5545 RRULE remains historical: moved instances are marked locally modified and ICS export flattens them instead of falsely exporting the unshifted RRULE or resurrecting canceled events.
- New Rust tests for UTC shifts, stale preview rejection, leap-day/exclusive all-day spans, cancellation preservation, edited/TZID refusals and date bounds; new browser UI regression for preview/cancel and honest native-only scope. Does NOT claim complete schedule editing, RECURRENCE-ID overrides or real native Windows calendar UI QA.
- Windows x64 unsigned NSIS/portable and Chromium screenshot CI gate required. The original 38-category REA weighted parity stays **28.9%**; Calendar remains Partial.

## 0.22.0 — Ambiguity-safe Windows GSMTC session controls (Batch Workstream D)

- WinRT GSMTC SourceAppUserModelId is not a per-tab identity. Detect duplicates in up to 64 sessions, flag ambiguous entries, disable transport controls for that ID, and reject selected snapshots or controls rather than targeting an arbitrary first browser tab.
- Refuse control and explicit-source reads when any Windows session identity cannot be verified. Check current transport capabilities immediately before sending a command; no microphone, media capture, network or privileged Windows API added.
- Media panel explains ambiguous/disconnected sessions and never silently controls a different player. Ignore out-of-order selected-player snapshots and bound extrapolated progress to 10 seconds to prevent misleading progress after sleep/resume. Clear stale artwork/progress when selection becomes ambiguous.
- Add Rust unit regression for multiple tabs sharing an app ID, and a Chromium fake-WinRT integration test for duplicate app controls, state, stale selection, and bounded progress. Physical Windows 10/11 Spotify, browser and Windows Media Player testing remains NOT TESTED.
- Unsigned Windows x64 installers remain the default; fixed 38-feature REA parity remains 28.9% (Music Partial).

## 0.21.0 — Stable selected-monitor identity and legacy-safe fallback (Phase 18C)

- Identify selected Windows/Tauri monitors with a bounded, local unique device name when available instead of relying on virtual-desktop origin coordinates. Moving a named monitor to another edge, swapping primary, or adjusting DPI no longer changes this saved selector as long as the OS reports the same unique name.
- If the OS reports an unnamed, duplicated, overly long or unsafe name, continue using the existing coordinate-based monitor identity. Recognize previously saved `origin:X:Y` preferences without migrating or reassigning them silently; UI invites explicit selection to adopt a stable name.
- Keep fallback to current primary when selected display is unavailable, preserve saved preference for reconnection, avoid altering OS display settings or stealing focus. Detect display-name changes during topology polling even if geometry stays unchanged.
- Add deterministic Rust unit regressions for stable monitor names, signed virtual origins, duplicate/unsafe names and preference syntax. Physical Windows 10/11 1–3 monitor hotplug/DPI/RDP/fullscreen QA is **NOT TESTED**.
- Source v0.21.0, unsigned x64 NSIS/portable subject to GitHub Actions. Weighted fixed 38-category parity still 28.9%; monitor behavior remains Partial pending real on-device acceptance.

## 0.20.0 — Local task priority, due date and non-destructive edit (Phase 18F)

- Extend local tasks with backward-compatible priority (low/normal/high, default Normal) and optional due timestamp with year 2000–2100 bounds. Old tasks, completed status, unique IDs, creation dates, settings and backups remain readable.
- Add validated task title/priority/due update command with atomic local transaction and rollback on failed writes; task editing preserves ID and completion state.
- Upgrade Tasks panel with original dark/champagne-gold responsive controls, add/edit priority selector and local datetime, due/overdue metadata, keyboard labels and cancelable editor; browser preview mirrors native state safely.
- Add deterministic Rust tests for legacy migration, due bounds, edit transaction, restart and rollback; new Chromium/Playwright end-to-end task form regression with local reload, completion and cancel, part of full CI alongside 64 screenshots.
- Task due notifications, recurring task schedule, bulk undo, task sync and natural language recognition are not implemented. Native interactive Windows 10/11 QA NOT TESTED. Fixed 38-category weighted reference parity remains 28.9%.

## 0.19.0 — Persisted bounded Windows reminder retries (Phase 18.1B)

- Added an opt-in native notification sender failure journal separate from successful enqueue reservations. Windows enqueue errors are retried after 20, 60 and 180 seconds, at most three total attempts; retries do not extend the existing time-based event grace period.
- The retry journal persists across process restart and is conservatively merged on backup restore so restoring an old snapshot does not unleash repeated OS sender calls. Successful reattempts remove stale retry records without invalidating their durable enqueue reservation.
- Strict per-event retry count and bounded map size, sanitized entries, and generic sender errors without private titles or raw source UIDs in logs. The Windows OS may still suppress accepted notifications, and an ambiguous OS failure can cause a limited repeat attempt: not exactly-once user-visible delivery.
- Settings now display pending/exhausted sender retry counts and maximum attempts without exposing event metadata. Added deterministic Rust restart, privacy, timing and restore tests; no new OS privileges or third-party notification reading.
- Entire feature parity remains the original fixed 38-category weighted 28.9%; interactive Windows 10/11 notification/Focus Assist/DND QA is NOT TESTED.

## 0.18.0 — Verified ICS recurrence-master export (Phase 18.1A)

- Preserve one original validated RRULE/EXDATE/RDATE ICS master VEVENT for a completely unchanged imported recurrence series, using strict re-expansion to compare every local occurrence, title, description, interval, timezone, all-day range, timed end and reminder.
- If any occurrence was edited, deleted, canceled, or lacks reliable persisted source metadata, explicitly fall back to independent flattened events; never export a master alongside duplicate materialized occurrences or recreate a deleted event on round-trip.
- Retain source notification reminders in new recurrence metadata; pre-0.18 series with a non-recoverable alarm fall back safely without losing reminders. Source input is folded with control-character rejection to prevent ICS property injection.
- Added five Rust regression tests for source-master round-trip, individual edits, tombstones, original timezone/monthly recurrence, legacy alarms and malformed source metadata. Local-only manual copy/paste export remains; native file picker and RECURRENCE-ID override semantics not yet available.
- CI/Windows hardware: acceptance depends on current PR and post-merge GitHub Actions; real interactive Windows 10/11 QA NOT TESTED. Fixed 38-category weighted parity remains 28.9%.

## 0.17.0 — Bounded monthly/yearly RFC5545 recurrence + versioned source series (development)

- Expand bounded DAILY/WEEKLY/MONTHLY/YEARLY RRULE with COUNT/UTC UNTIL/INTERVAL, BYDAY (including supported month-relative ordinals), WKST, BYMONTH and BYMONTHDAY (negative indices); reject unsupported parts rather than silently misreading. Leap days, missing month-end days and DST ambiguity are covered by Rust tests.
- Persist normalized ICS source metadata v1 alongside materialized instances: original DTSTART, DTEND/DURATION, RRULE, EXDATE/RDATE, user-selected floating timezone and per-instance edit markers. Legacy calendars remain backward compatible; no false reverse-inference from old instances.
- Per-instance edits retain distinct identity and record override markers; series title rename updates saved source metadata; deleting a series keeps its durable title-free cancellation marker and drops the source metadata. Reimport never overwrites an existing source or local edits without explicit user action.
- Calendar UI labels saved recurrence source rules or truthfully identifies legacy source metadata as unavailable. New Playwright regression covers scoped edits, renaming and cancellation; original 16 module UI remains.
- Original ICS export still deliberately flattens instances; source-preserving RRULE export, RECURRENCE-ID overrides, full series schedule changes, native file picker, cloud calendar sync and real interactive Windows QA remain outstanding. REA 38-category weighted parity stays 28.9%.
- Phase18 next prompt is a much larger multi-workstream batch spanning calendar, reminders, monitors, GSMTC, UI/UX, productivity, Windows controls and package recovery.

## 0.16.0 — Timed UTC event ends and durable ICS cancellation (development)

- Added optional UTC exclusive timed event end (max 366 days), strict IANA-aware DTEND and positive DURATION subset import/export, DST gap/overlap checks and backward-compatible JSON serialization.
- Recurrence expansion preserves elapsed UTC durations after timezone-specific occurrence starts. Full source recurrence metadata and wall-clock schedule editing remain out of scope.
- Added bounded, title-free local cancellation markers for deleted imported occurrences or entire series. Explicit user-confirmed reset allows a later manual reimport; no background subscriptions or unintentional resurrection.
- Timed start/end and private description editing, DST-safe multi-day month badges, new Rust/Playwright regressions, and read-only native Windows acceptance script.
- Unsigned Windows x64 NSIS and portable deliverables require CI; actual interactive Windows 10/11 test remains NOT TESTED. Weighted 38-category parity remains 3 Implemented / 16 Partial / 19 Gap = 28.9%.

## 0.15.0 — Civil-date calendar and series actions (development)

- True iCalendar all-day DTSTART;VALUE=DATE/DTEND;VALUE=DATE import/export with exclusive end dates, timezone-neutral display, non-destructive UTC legacy state compatibility and no automatic midnight reminders.
- Simple bounded DAILY/WEEKLY RRULE exceptions via EXDATE/RDATE (both date-only and timed), stable source UID + per-instance identities and repeat-import deduplication.
- Transactional rename-all-occurrence titles and remove-entire-imported-series actions; per-event edit remains independent. Removal is local, and re-import can restore the series. Complex source recurrence/series schedule edits remain out of scope.
- All-day create/edit UI, multi-day month badges, clear exclusive-end labeling, accessible series actions and new Playwright regression for preview mode.
- Date-only/leap/DST test coverage, 38-category parity still 3 Implemented / 16 Partial / 19 Gap = 28.9%. Native Windows 10/11 interactive tests remain NOT TESTED; installer and portable builds are unsigned.

## 0.14.0 — Safe IANA calendar import and native QA inventory (development)

- Added a separate Rust timezone/recurrence module for iCalendar DTSTART with UTC, IANA TZID and explicitly chosen floating IANA timezones. DST nonexistent/ambiguous local instants are rejected rather than silently shifted; UTC persisted event schema remains unchanged for existing backup/reminder migration.
- Import bounded DAILY/WEEKLY RRULE with COUNT or UTC UNTIL and optional INTERVAL; materialize up to 250 standalone UTC occurrences with stable per-occurrence identifiers to keep repeated imports idempotent. Reject all-day VALUE=DATE, complex RRULE, EXDATE/RDATE/RECURRENCE-ID and unbounded recurrence rather than silently truncating.
- Added deterministic tests for Asia/Jakarta offsets, New York DST rollover/gaps/overlaps, recurring occurrence identities, JSON state preservation, UTC export and input bounds.
- Added a clearly labeled floating-timezone field to calendar ICS import UI, with no implicit OS/device-timezone conversion.
- Added a local-only PowerShell acceptance inventory and manual Windows 10/11 native QA checklist; do not confuse hardware inventory or green CI packaging with interactive Windows behavioral proof.
- 38-category parity remains **3 Implemented, 16 Partial, 19 Gap = 28.9%**; native WebView2, monitor hotplug and GSMTC physical tests remain NOT TESTED.

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

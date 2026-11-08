# WFS Notch Phase 14 — Timezone-safe Calendar & Native Acceptance Protocol

**Project:** WFS Notch, Rust + Tauri 2, Windows 10/11.  
**Baseline:** main 08c6afd4aaa940e72013b713564b852baddfc280 (Phase 13 all CI gates PASS).  
**Development branch:** feat/phase14-calendar-timezones-recurrence.  
**Target source version:** 0.14.0, unsigned Windows installer and portable executable (if current-head CI passes).

## Implemented source

1. Isolated chrono + chrono-tz IANA parsing in src-tauri/src/ics_time.rs. DTSTART supports existing UTC timestamps, explicitly named IANA TZIDs (for example Asia/Jakarta, America/New_York) and floating local date-times ONLY with a user-provided IANA timezone. The Rust state schema persists each event as an absolute UTC epoch millisecond, so old backups and event reminder keys remain compatible.
2. Resolve daylight-saving spring-forward missing wall times and fall-back ambiguous instants explicitly as errors. No silent hour shift or OS timezone guess. Zoned recurring events are advanced in wall-clock local dates rather than adding fixed 24-hour UTC periods.
3. Support bounded RFC 5545 simple RRULE FREQ=DAILY/WEEKLY with INTERVAL, COUNT or UTC UNTIL. Reject COUNT greater than 250, invalid/unbounded rules, BYDAY/BYMONTH/complex recurrence and import exceeding a 250-event batch. Atomic parser validation prevents half-import.
4. Materialize recurrences as individual standalone UTC CalendarEvent records with stable per-occurrence IDs. Reimport of the same UID and schedule skips duplicates; editing or deleting one occurrence does not edit the full source series. ICS export writes each event as a UTC standalone event; original RRULE is **not** preserved. No background calendar network sync.
5. Explicitly reject unsupported all-day VALUE=DATE (no all-day schema), EXDATE, RDATE, RECURRENCE-ID and unknown DTSTART parameters. Unrecognized complex calendars must be handled by a future full RFC 5545 engine instead of approximated.
6. Added a user-visible IANA floating-timezone input with clear privacy/precision copy, Rust DST/recurrence and JSON legacy-state tests, and a local-only Windows acceptance helper at scripts/phase14-native-acceptance.ps1.

## Limitations and separation of evidence

**Native Windows interactive tests:** NOT TESTED on physical Windows 10/11. The local PowerShell helper creates an inventory and a manual QA checklist with all statuses NOT TESTED. It neither installs software nor captures notification bodies, private data or screenshots. It does not prove hardware behavior.

**Automation:** GitHub CI covers Rust, Clippy, rustfmt, headless Chromium browser regressions and screenshot evidence, and Windows MSVC NSIS/portable packaging. See current PR and workflow evidence for actual PASS/FAIL. A green Windows build is not a hardware, Focus Assist, WebView2, monitor-hotplug or GSMTC live acceptance test.

**Golden screenshots:** No approved WFS-owned visual references are currently provided for pixel-diff assertions. The screenshot matrix is diagnostic headless-browser evidence, not a pixel-golden comparison.

**Exactly-once reminder visibility:** NOT SUPPORTED. The durable ledger prevents repeated enqueue attempts within retention but cannot prove OS display or user sight.

**Chronology:** TZID parsing uses the packaged IANA timezone database snapshot; future political timezone-rule changes depend on dependency updates. Floating times are resolved only with explicit user selection. All-day, monthly/yearly, complex RRULEs and original series metadata remain gaps.

## Manual Windows test protocol

Run in an interactive Windows 10/11 session (PowerShell, no elevation), supplying paths if available:

    powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\phase14-native-acceptance.ps1 -InstallerPath "C:\path\WFS Notch_0.14.0_x64-setup.exe" -PortablePath "C:\path\wfs-notch.exe"

The local Markdown report is saved under %LOCALAPPDATA%\WFS-Notch\NativeQA. Confirm real installed/portable behavior, per-app toast settings and Focus Assist, sleep/resume, DST, monitor 1–3 topology/hotplug/DPI, RDP, GSMTC Spotify/browser/player, reduced motion and native WebView2 with sanitized manual evidence. Do NOT upgrade any NOT TESTED status based on the inventory alone.

## 38-category reference coverage

Implemented 3, Partial 16, Gap 19; exact weighted numerator = 3 + 16 × 0.5 = 11. Weighted progress = 11 / 38 × 100 = **28.9%** (one decimal). Desktop calendar remains Partial: all-day dates, complete RFC recurrence, series editing, subscriptions and cloud interoperability are not completed; no new category is promoted by a parser improvement.

## CI and merge policy

Branch PR must pass current-head Rust unit tests, Clippy -D warnings, rustfmt, all browser tests including 64 screenshot matrix, Windows NSIS and portable unsigned packaging. Inspect the latest SHA and all required jobs; do not merge if any failed, cancelled or outstanding. After merge verify the main push identity job and package artifacts again. No force push and no proprietary reference source/assets or public tunnel.

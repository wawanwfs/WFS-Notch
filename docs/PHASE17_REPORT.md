# WFS Notch Phase 17 — RFC5545 Recurrence Rules & Versioned Series Metadata

Repository: https://github.com/wawanwfs/WFS-Notch  
Baseline main: eda48670d962737b51b89f207a397aeeef901ef8 (v0.16.0, final CI 42497020272 ALL PASS).  
Feature branch: feat/phase17-recurrence-metadata-rfc5545.  
Source version: 0.17.0. All release claims require a fresh successful exact-head PR CI and final main workflow.

## Scope actually implemented

1. Bounded RFC5545 recurrence engine: DAILY, WEEKLY, MONTHLY, YEARLY with INTERVAL, COUNT (1–250), explicit UTC UNTIL, BYDAY (weekday and month-relative ordinals when meaningful), BYMONTH, BYMONTHDAY (including negative from end of month), and WKST. Case-insensitive weekday tokens; duplicate/invalid/unsupported rule parts rejected rather than ignored. No silent truncation or weekend/holiday guesses.
2. Preserve wall-clock start across IANA DST transitions and maintain explicit user-selected timezone for floating starts. Missing/ambiguous DST wall times fail closed. Monthly 31st skips months without day 31 instead of clamping; Feb 29 yearly recurrence skips non-leap years. Calendar date range and expanded occurrence count remain bounded.
3. Persist **versioned v1 normalized imported source-series metadata**, with original DTSTART/DTEND/DURATION, RRULE, EXDATE/RDATE, floating timezone, title, description, stable series UID, and explicitly modified occurrence IDs. Local JSON state and backups preserve the metadata. Legacy materialized occurrences remain usable but **no original RRULE is invented** from their timestamps.
4. Phase16 tombstones still prevent accidental reimport of deleted imported occurrences or series. Explicit series deletion drops retained source metadata and persists the cancellation marker. Single-occurrence edits are recorded against known source metadata, series title rename updates source metadata but never shifts unrelated occurrences. Reimports retain already stored source metadata and local edits; automatic source-series schedule overwrites are not performed.
5. The existing 16-module dark/gold WFS UI and local preview keep separate single occurrence edit vs entire series title/delete choices. The calendar displays the saved source recurrence rule for new imported series and labels legacy/unknown series honestly.
6. Rust tests cover month-end, leap-year, ordinal BYDAY, BYMONTH, BYMONTHDAY, WKST, DST, bounded expansion and invalid inputs; source series serialization, local edits, reimport, rename/delete and cancellation. A new Playwright Phase17 scenario checks UI source-rule metadata, single-instance edit marker, series title scope and delete behavior.

## Explicit limitations and unresolved work

- **Physical interactive Windows 10/11 testing: NOT TESTED.** GitHub Windows CI compiles/packages binaries but does not verify NSIS upgrade/install behavior, OS Focus Assist/DND/toast visibility, sleep/clock changes, 1–3 physical monitors, hotplug/DPI/RDP, actual GSMTC Spotify/browser/Media Player, or native WebView2 accessibility.
- **Original RRULE serialization on ICS export: NOT IMPLEMENTED.** The exporter still writes flattened occurrences; source RRULE remains in local private metadata, not yet round-tripped into an exported recurrence master.
- **Source schedule-aware series edit/reflow: NOT IMPLEMENTED.** Rename/delete series and edit one occurrence remain supported; regenerating occurrences without losing user edits requires a separate safe plan.
- **RECURRENCE-ID override semantics: NOT IMPLEMENTED** and fail closed. VTIMEZONE custom aliases/unknown timezone definitions, BYSETPOS, BYWEEKNO/BYYEARDAY, year-relative numbered BYDAY and other unsupported combinations are rejected rather than silently misinterpreted.
- **Native user-selected ICS open/save file picker: NOT IMPLEMENTED.** Existing paste/copy is local-only, 256 KiB import bound; no remote subscriptions, Google/Outlook sync or undisclosed file access.
- **No golden screenshot pixel comparisons:** approved WFS-owned reference images not available. Existing 64 screenshots are Chromium regression evidence, NOT actual WebView2/native behavioral evidence.
- **No exactly-once Windows reminder display** guarantee. Ledger is bounded at-most-once OS enqueue attempt, not OS delivery/read receipt. No code signing or unlicensed third-party source/assets.

## Automated acceptance and evidence

At the exact PR head and final merged main: Windows \`cargo test --all-targets --verbose\`, Clippy \`--all-targets -- -D warnings\`, rustfmt \`--check\`, Node syntax/static audit, all Playwright including Phases 15/16/17 and 64 Chromium screenshots, unsigned Windows x64 NSIS and portable artifacts, plus final main push owner identity audit. The PR must stay draft on failing gates and only merge when all required PR-head checks PASS. Final main GitHub Actions URLs, SHA and artifacts are required separately.

## Native QA instructions

On a real authorized Windows 10/11 machine, use \`scripts/phase16-native-acceptance.ps1\` (local-only read-only inventory and user-entered status), verify the exact final build installer SHA256 and portable SHA256, then manually test NSIS, toast privacy/DND, sleep/resume, fullscreen/DPI/hotplug, RDP, GSMTC, WebView2 keyboard/screen reader and calendar recurrence. Do not mark PASS based on headless CI or screenshots. Never upload user filenames, notification text or private calendar contents.

## Fixed REA weighting

38-category source matrix remains 3 Implemented, 16 Partial and 19 Gap: **28.9% weighted parity**, unchanged. Calendar remains Partial because full source reflow, exceptions, cloud permissions, native device QA and reference parity are incomplete. Phase17 adds meaningful capability within that Partial category, not a newly finished top-level category.

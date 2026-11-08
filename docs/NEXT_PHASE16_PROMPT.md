# WFS Notch — Phase 16 Full Execution Prompt

Continue from the ACTUAL latest main commit in https://github.com/wawanwfs/WFS-Notch. Audit GitHub Actions, PR history, README, docs/PHASE15_REPORT.md, docs/feature-coverage.md, docs/security.md, native modules main/calendar_ics/ics_time/all_day/notifications/media_sessions/display, frontend state and all browser tests. Preserve WFS dark/champagne-gold branding, Rust + Tauri 2, Windows 10/11, secure local state, reminder privacy, backup safety and the original working 16-module interface.

## P0 — Real Windows interactive verification (no fabricated results)

1. On authorized physical or interactive Windows 10 and Windows 11 systems, validate the current unsigned NSIS installer and portable .exe separately, including install/uninstall, upgrade from old state, startup and process lifecycle. Collect sanitized LOCAL QA evidence; do not upload notification body, user files or titles.
2. Run read-only scripts/phase14-native-acceptance.ps1 for basic display inventory only, then verify each manual test. Compare native WebView2 keyboard, screen reader, hover, reduced motion and contrast against browser results. No approved golden screenshot baseline means NO pixel-golden comparison.
3. Test OS per-app toast permissions, private redaction, DND/Focus Assist, lock screen, sleep/resume, system time rollback, backup restore and edited/cancelled reminder dispatch. Existing durable ledger only provides best-effort at-most-once OS enqueue attempts, not exactly-once visible delivery.
4. Test 1–3 real monitors, primary/secondary fullscreen/borderless/maximized, DPI 100/125/150%, negative virtual origins, resolution change, hotplug and RDP disconnect/reconnect, without focus stealing or invasive input hooks.
5. Test GSMTC Spotify, supported browsers and Windows Media Player sessions for source selection, artwork, progress, inactive controls and disconnects; AppUserModelId may not distinguish multiple instances.

## P1 — Complete calendar semantics safely

6. Preserve Phase15 all-day start/exclusive-end, timezone-neutral display and legacy UTC data. Add true timed DTEND/duration in UTC/IANA with DST proof and round-trip; currently deliberately rejected.
7. Advance recurring-series metadata from materialized occurrences to a durable source series with schedule-aware edit/cancel, exception tracking, idempotent tombstone decisions and backward-compatible migrations. Existing Phase15 group commands change titles and remove current local instances only; explicit reimport can restore deleted items.
8. Support more RFC 5545 recurrence (BYDAY/BYMONTH, monthly/yearly) and validated RECURRENCE-ID overrides with safe expansion bounds. Do NOT silently drop unsupported rule components. VTIMEZONE aliases only when provable; floating times require explicit user timezone.
9. Consider native open/save picker only through official scoped Tauri APIs, explicit user consent, restricted file content/size and least-privilege capabilities. No background cloud calendar subscriptions until privacy/consent are implemented.
10. Refine all-day date picker and scoped occurrence-versus-series editing with accessibility and consistent browser/native state. Add deterministic leap-day and DST gap/overlap regression tests and protect existing event/reminder and backup behavior.

## Development quality, release and ownership

- Create a new feat/phase16-... branch from the actual latest main, make working production Rust/JS/CSS code with tests and documentation. No force push, no proprietary vendor assets/source, no public tunnel, no unlicensed lyric scraping.
- Require Windows cargo test --all-targets --verbose, cargo clippy --all-targets -- -D warnings, cargo fmt --all -- --check, Node/structural audits, ALL Playwright suites including Phase15 all-day/series browser test and 64 browser screenshots, Windows x64 unsigned NSIS and unsigned portable output.
- Report native hardware tests and browser UI tests separately. If interactive Windows machines are unavailable explicitly label them NOT TESTED.
- Keep original 38-category weighted matrix: Implemented=1, Partial=0.5, Gap=0. Phase15 baseline 3 Implemented, 16 Partial, 19 Gap = 28.9%. Only promote on independent complete evidence; never claim 100% merely from green CI.
- Open PR to main, inspect the exact most recent head SHA and every required job. Merge only if ALL PASS; after merge verify main push CI plus owner attribution. Provide main SHA, PR/run links, SHA256, unsigned NSIS+portable artifacts and screenshots.
- Deliver docs/PHASE16_REPORT.md and docs/NEXT_PHASE17_PROMPT.md, honest unresolved gaps, and executable local Windows QA instructions.

**Priority:** functional code and verified safeguards over unverifiable parity claims.

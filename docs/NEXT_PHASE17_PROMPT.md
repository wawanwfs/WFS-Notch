# WFS Notch — Phase 17 Next Execution Prompt

Repository: https://github.com/wawanwfs/WFS-Notch, branch main. Read ACTUAL latest SHA and all GitHub Actions. Audit docs/PHASE16_REPORT.md, docs/feature-coverage.md, docs/security.md, README and the native Rust main/calendar_ics/ics_time/all_day/notifications/media_sessions/display modules, UI and browser tests. Preserve Rust + Tauri 2, Windows 10/11, local-first user privacy and WFS premium dark/champagne-gold branding. Do not reimplement working modules.

## P0: Hardware acceptance, real evidence

1. On physical or authorized interactive Windows 10 and Windows 11 machines (NOT CI headless), validate unsigned installer and portable binaries, update and migration, backup restore, tray/startup. Run privacy-preserving scripts/phase16-native-acceptance.ps1 and record explicit sanitised manual evidence. If interactive machines cannot be accessed, report NOT TESTED, do not invent a PASS.
2. Verify toast OS permission and DND/Focus Assist, redacted/private mode, sleep/resume and time changes, reminder ledger, and cancellation/backup behavior. At-most-once OS enqueue is not exactly-once visible notification receipt.
3. Test 1–3 monitors with fullscreen/borderless/maximized, 100/125/150% DPI, negative origins/hotplug/RDP without focus stealing; GSMTC real Spotify, browsers and Windows Media Player; actual WebView2 accessibility, keyboard, reduced motion, hover and system contrast. No OS hooks or admin privileges for basic inspection.

## P1: Truly persistent RFC5545 recurrence

4. Phase16 supports true timed exclusive DTEND and positive elapsed DURATION, along with all-day DATE/DTEND. Keep strict UTC/IANA validation, DST proof, 366-day bounds and non-destructive old backups.
5. Replace flatten-only source series with a versioned durable recurrence metadata model: source UID, DTSTART zone, normalized RRULE, EXDATE/RDATE, recorded per-occurrence edits, event start/end semantics and cancellation tombstones. Migration must preserve any manually edited materialized occurrence and all current cancellation markers. Do not guess source rules from existing materialized instances.
6. Expand safe RFC5545 BYDAY/WKST, monthly/yearly FREQ and BYMONTH/BYMONTHDAY with correct skipping semantics for short months/leap dates, COUNT/UNTIL and deterministic bounded expansion. Implement RECURRENCE-ID overrides and cancellation only if the data model preserves meaning atomically; otherwise explicitly reject unsupported inputs with no partial state changes. Test New York DST overlap/gap, Asia/Jakarta, Feb29/Feb30 and time bounds.
7. Make all-series vs one-occurrence edits explicit and undoable where appropriate; schedule-aware series reflow must not silently shift an unrelated event or re-enable canceled events. Make cancellation-marker clearing deliberately confirmed, bounded and private.
8. If practical, implement native open/save ICS dialogs using official Tauri2 plugin/capabilities and user-selected paths only. Do not fetch remote subscription data without explicit consent or add Google/Outlook credentials as a shortcut.

## P2: Accuracy, QA and signed-release readiness

9. Add more deterministic Rust tests, Playwright and accessibility checks, plus a local-only device QA script. Require CI 55+ Rust tests, cargo clippy --all-targets -- -D warnings, cargo fmt --all -- --check, Node syntax/structural validation, Phase15/16 browser regressions, 16 modules x four configurations = 64 screenshots, Windows x64 NSIS+portable package artifacts.
10. Track original REA-derived 38 categories, baseline Phase16 3 Implemented, 16 Partial, 19 Gap = 28.9%. Upgrade category only when genuinely complete. Keep physical Windows QA NOT TESTED until observed. Strict visual golden diffs only when approved WFS-owned screenshots are supplied.
11. New feat/phase17-... branch, PR to main, no force push, merge only if all latest-head CI gates PASS. Verify final main push identity, CI, checksum and unsigned package artifacts; do not claim code signing exists. Deliver docs/PHASE17_REPORT.md, docs/NEXT_PHASE18_PROMPT.md, PR/merge SHA/CI links, unresolved risks and realistic parity percentage.

No proprietary Poni executable or copyrighted UI assets, unauthorized reverse engineering, unlicensed lyrics, cloud upload of private data, public tunnels, or insecure OS privileges.

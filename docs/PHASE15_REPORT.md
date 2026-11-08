# WFS Notch Phase 15 — Calendar Civil Dates, Series Operations & Native QA

**Repository:** https://github.com/wawanwfs/WFS-Notch  
**Baseline:** main SHA 0e671543840f585c87b707d93227c16b4218056f, Phase14 run 42490445081 ALL PASS.  
**Branch:** feat/phase15-all-day-series. **Development version:** 0.15.0, unsigned Windows x64.

## Production source changes

1. Added AllDaySpan optional serde metadata on CalendarEvent: timezone-neutral ISO start and exclusive end civil dates, 1–366 days, leap-day support and UTC-midnight numeric storage anchor ONLY. Existing JSON/backup/timed events remain backward compatible and all-day events have no implicit midnight reminder.
2. RFC 5545 ICS DATE/DTEND import and export for all-day events, including one-day default if DTEND is absent, exclusive end validation and multi-day spans. All-day UI creation/edit and date-neutral month badges added. Timed DTEND is explicitly rejected until duration can be modeled.
3. Bounded DAILY/WEEKLY recurrence now recognizes EXDATE and RDATE for DATE and DATE-TIME, with user-selected timezone for floating times and strict DST gap/overlap errors. Max import batch 250; source series must have stable safe UID. RECURRENCE-ID overrides and complex recurrence are rejected rather than silently misread.
4. Persist source series UID per materialized occurrence. Supports transaction-safe rename-all-occurrence titles and delete-all-occurrences actions, with visible controls, confirmation and preview parity. Single-instance edit is separate. Full-series schedule shifting, exception editing and recurrence rules are still NOT IMPLEMENTED. Explicit reimport can restore deleted occurrences because cancellation tombstones do not exist.
5. Reimport of a Phase14 series detects former first-event legacy UID and avoids duplicate occurrence, optionally enriching previously ungrouped entries with source series identity; old UTC data is not migrated destructively.
6. New Playwright tests cover local browser all-day create/edit and multi-day badges, series rename and delete, and existing browser+static QA remains mandatory. Rust tests cover civil dates/invalid spans/leap day, DATE/DTEND roundtrip, EXDATE/RDATE across DST, idempotency, series updates/deletes and legacy JSON.

## Explicit testing boundaries

- **Physical interactive Windows 10/11 QA: NOT TESTED.** This execution environment cannot prove toast visibility, DND/Focus Assist, sleep/resume, real monitor hotplug, fullscreen/focus, RDP reconnect, GSMTC Spotify/browser/Media Player, or actual WebView2 accessibility. Use the local-only read-only script scripts/phase14-native-acceptance.ps1 on real machines to generate a manual checklist; manually collect sanitized evidence before changing NOT TESTED statuses.
- **Browser UI evidence** comes from headless Chromium; it cannot prove native WebView2 or OS APIs.
- **Golden-image pixel compare: NOT TESTED.** Approved WFS-owned reference screenshots are unavailable.
- **Exactly-once visible Windows reminders: NOT SUPPORTED.** Existing at-most-once durable OS-enqueue ledger does not prove visible display or a user's observation.
- **Calendar partials:** no original RRULE serialization, complex BYDAY/BYMONTH, monthly/yearly, timed DTEND, RECURRENCE-ID overrides, full-series schedule edit, cancellation tombstones, complete VTIMEZONE alias parsing, native open/save picker, subscriptions or Google/Outlook cloud sync.
- No proprietary vendor source/assets, public tunnel, global input hooks or unlicensed lyrics were added.

## Automated acceptance required

At the exact PR-head commit and then final main commit verify Windows Rust --all-targets tests, Clippy -D warnings, rustfmt check; Node syntax and Playwright browser regressions including new Phase15 calendar scenario; 16 modules x 4 screenshot matrix; and Windows x64 NSIS plus unsigned portable executable artifact outputs. Keep any failing PR unmerged.

## Native Windows acceptance (manually required)

| Scenario | Windows 10 | Windows 11 |
| --- | --- | --- |
| NSIS install/uninstall and portable launch | NOT TESTED | NOT TESTED |
| Toast permission, privacy, DND and locked screen | NOT TESTED | NOT TESTED |
| Sleep/resume, time changes and backup restore | NOT TESTED | NOT TESTED |
| 1–3 monitors, fullscreen/maximized, hotplug and DPI | NOT TESTED | NOT TESTED |
| RDP reconnect and focus behavior | NOT TESTED | NOT TESTED |
| GSMTC real-player selection/control/progress/artwork | NOT TESTED | NOT TESTED |
| WebView2 keyboard, reduced-motion, contrast and hover | NOT TESTED | NOT TESTED |
| All-day calendar and DST behavior on device | NOT TESTED | NOT TESTED |

## 38-category REA-derived weighted coverage

Implemented=3, Partial=16, Gap=19. Formula: (3 + 16 x 0.5) / 38 x 100 = **28.9%**. Desktop calendar remains Partial; backend/parser improvements do not complete calendar subscriptions, native device QA or all reference behavior. No category is promoted without full evidence.

## Release discipline

Do not force push. Merge only on current-head green PR CI, then confirm exact main commit has all four jobs PASS including owner identity gate. Report only verified installer/portable artifact URLs from successful main CI and warn that both binaries are UNSIGNED.

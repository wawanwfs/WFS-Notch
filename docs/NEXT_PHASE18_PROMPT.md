# WFS Notch — Phase 18–22 Accelerated Multi-Workstream Execution Prompt

> One large execution batch instead of many tiny prompts. Actively execute as many independent workstreams as safely possible during the current working session, without repeatedly asking the user to type "lanjutkan". Split implementation into reviewable PRs and merge ONLY on green latest-head CI. Never promise asynchronous execution or invent native Windows hardware results.

Repository: https://github.com/wawanwfs/WFS-Notch
Stack: Rust + Tauri 2, Windows 10/11, local-first, privacy-preserving premium WFS dark/champagne-gold brand.
Starting point: read ACTUAL latest main SHA and latest GitHub Actions results. Audit README, docs/PHASE17_REPORT.md, docs/PHASE16_REPORT.md, docs/feature-coverage.md, docs/security.md, all UI/native source, tests and Actions. Preserve all 16 existing modules and do not recreate working functions.
Baseline: Phase17 source v0.17.0 if merged. The fixed original REA matrix starts with 3 Implemented, 16 Partial, 19 Gap = 28.9% weighted coverage until independently evidenced otherwise.

## Global rules — mandatory

1. Execute implementation, not only brainstorming. Parallelize audit and independent modules where possible, but serialize writes to the same file/ref and create independent feature branches/PRs from current main. No force pushes, destructive resets, unsafe source copying or false success.
2. After each successful full PR-head CI, merge with expected SHA, verify final-main CI and push identity, THEN continue the next feasible workstream within the SAME active execution. Do not require a follow-up message after each PR. Do not assert that work can keep running after this session ends.
3. Never merge red CI. Require Windows cargo test --all-targets --verbose, cargo clippy --all-targets -- -D warnings, cargo fmt --all -- --check, Node syntax/static audits, all Playwright suites, 64 Chromium screenshot matrix (16 modules x 4 configurations), Windows x64 unsigned NSIS + portable artifacts and checksum from the exact commit.
4. Do not substitute headless CI for interactive Windows 10/11 hardware acceptance. Preserve NOT TESTED for actual toast visibility/DND/Focus Assist, sleep/resume, multi-monitor/hotplug/RDP, GSMTC real Spotify/browser/Windows Media Player and native WebView2 keyboard/accessibility until genuinely observed. Do not fabricate evidence.
5. Protect state.json, backups and migrations, timer/reminder ledgers, calendar UTC/DST/all-day/series identity/cancel tombstones, user task/note data, and privacy settings. Each migration has rollback and tests. OS settings and cloud connections require informed consent, never silent elevation.
6. Use REA/LeanREA/Craft Anything skills ONLY if legally available and actually installed; otherwise use source/spec/CI analysis and original implementation. No proprietary Poni code/assets or pixel-for-pixel imitation. No unlicensed lyrics, leaked credentials, invasive OS hooks or public tunnels.
7. Distinguish per-workstream engineering progress from the fixed 38-category weighted parity. Do not increase category score without complete behavioral evidence and native tests where necessary.
8. Maintain audit-ready docs and a cumulative progress scoreboard after each merged PR, including commit SHA, PR, test counts, CI run, artifacts, SHA256, new features, unfinished gaps and native QA status. Never say "100% finished" because CI is green.

## Workstream A — Calendar full recurrence fidelity (P0)

Phase17 implemented local versioned source metadata and safe bounded DAILY/WEEKLY/MONTHLY/YEARLY expansion with BYDAY, BYMONTH, BYMONTHDAY, WKST; build on it.

- Preserve source master VEVENT with UID, DTSTART TZID/UTC/floating user timezone, RRULE, EXDATE/RDATE, DTEND/DURATION, modified occurrences and cancellation tombstones. Persist fingerprints/revisions without inferring missing rules from old flattened records.
- Implement source-preserving ICS export: one master RRULE+exceptions and separately representable RECURRENCE-ID overrides, with fail-closed fallback when a user edit cannot faithfully round-trip. Do NOT emit the master AND duplicate flattened events.
- Series schedule editor: user chooses edit this occurrence vs whole series; show exact preview of old/new dates, durations, collisions, DST failures and affected reminders; transactionally regenerate only eligible unedited instances, retaining local overrides, IDs and canceled markers. No silent restore of deleted events or duplicate toast reservation.
- Support RECURRENCE-ID overrides only with safe matching original start/UID, explicit timezone, and fully tested expansion. Reject unsupported RANGE=THISANDFUTURE and aliases rather than silently guessing.
- RFC5545 tests: Feb29, Jan31 skip, fifth weekday absence, last weekday, BYMONTHDAY=-1, BYDAY ordinal, WKST, COUNT/UNTIL, EXDATE/RDATE, TZID DST gap/overlap, double-import, backup/restore, 250-event boundary, atomic rejection of malformed sources.
- Consider a scoped native user-selected ICS open/save dialog using official Tauri 2 APIs; 256 KiB import limit, explicit user action and least privilege only. No cloud subscriptions without separate user consent.

## Workstream B — Native notifications & reminders (P0)

- Audit notification scheduler, durable at-most-once journal, edit/cancel/resume/restart/replay semantics and private title redaction; improve deterministic time injection, retry, permissions diagnostics and bounded backoff.
- Implement read-only Windows notification settings diagnostic and opt-in alerts, WFS-only pause, safe handling of DND/Focus Assist and lock-screen where permitted; never modify OS global notification policy.
- Test fake OS sender failure, reboot/crash after reservation, clock rollback, DST, backup restore and recurring event edits; preserve no private titles in logs. At-most-once OS enqueue is NOT exactly-once visible toast, and user-visible delivery cannot be automatically verified in CI.
- Provide real Win10/11 manual test checklist and no false PASS.

## Workstream C — Fullscreen / monitors / DPI / focus / RDP (P0)

- Improve monitor identity across unplug/replug and primary swaps; test 1-3 monitors, negative origins, fullscreen vs maximized/borderless, 100/125/150% DPI, Win32 scaling, hotplug, resolution change and RDP reconnect.
- Keep notch non-focus-stealing and frame-stable; no global keyboard hooks or admin rights. Handle selected-monitor loss and restore without jumping to an unrelated monitor silently.
- Add deterministic Rust display geometry tests, browser responsive regressions and separate native manual evidence. Chromium simulation does NOT prove Windows compositor behavior.

## Workstream D — GSMTC media parity (P1)

- Improve session discovery and selection, stale/disconnected sessions, source naming, capability-checked playback, seeking/progress drift, artwork invalidation and memory limits.
- Support Spotify, compatible browser sessions and Windows Media Player when APIs permit; do not pretend AppUserModelId uniquely identifies all tabs/instances.
- Add fake-session Rust tests and UI Playwright; actual real-player native QA remains NOT TESTED until performed.
- Synced lyrics require legitimate licensing/API terms and consent. No unlicensed copyrighted lyrics in code or replay UI.

## Workstream E — Premium WFS notch UI/UX & accessibility (P1)

- Audit ALL 16 module screens and original WFS identity, not only one pane. Upgrade original dark/champagne-gold 3D/glass aesthetics, consistent hierarchy/typography, modern iconography, animations, coherent loading/error/empty states, keyboard focus and tooltips.
- Responsive compact and expanded modes, minimal 760x470 viewport, reduced motion, contrast modes, focus rings, tab/keyboard order, labels and screen-reader announcements.
- Add user-customizable accent/density/animations with safe reset and persistent settings. No proprietary copied layouts or user-owned reference pixel golden tests without supplied/approved screenshots.
- Maintain and expand full Playwright regression suites; no decorative non-functional controls.

## Workstream F — Tasks, notes, focus, health (P1)

- Tasks: priority, edit, due date/time, recurring tasks with bounded schedule, snooze, reminders, clear completed with undo and keyboard shortcuts.
- Notes: tags/folders, search, sort, structured local export/import, save-conflict protection and undo; prevent HTML injection and always retain originals/backups.
- Pomodoro: complete work-break cycles, pause/resume, process restarts, sleep/clock changes, optional notifications and progress summary.
- Wellness: hydration/break opt-in reminders, quiet hours and local day rollover; never imply medical diagnoses.
- Each capability requires persisted data, UI, regression/rollback tests and privacy audit before marking implemented.

## Workstream G — Windows controls / system panels (P2)

- Read-only Wi-Fi/Bluetooth adapter and current status via official safe Windows APIs first. Implement toggles only with explicit consent and supported privileges; do not scan nearby user identities or change global configuration silently.
- Expand disk/network/CPU/RAM telemetry with GPU/temp only when safe; no invasive drivers. Constrain polling and CPU overhead; support disconnected devices.
- Implement only real, controllable dock/launch/shelf features; file drops and Clipboard History must have per-feature permission and redaction, not intrusive global surveillance.
- Test Windows-only code with mocks and give interactive QA gaps separately.

## Workstream H — AI integrations & optional connected services (P2, opt-in)

- Local Ollama chat/translation first if feasible. Third-party providers require explicit user consent, securely stored credentials via Windows Credential Manager/DPAPI, key revoke and no plaintext JSON. No fake provider-cost totals; log true usage only.
- AI plan/Claude Code/GitHub module need legitimate supported APIs and scopes. Never imitate app notifications without real integration.
- Online weather, calendar sync and event subscriptions require explicit privacy UX, limited scoped access and secure token handling; skip when credentials unavailable.
- Workstream H is NEVER prerequisite for local features or packaging.

## Workstream I — Build, installer, backup & release integrity (P1)

- Win10/11 x64 NSIS/portable and SHA256 from exact successful merged main SHA. Test upgrades from 0.15-0.17 and preserve local state and backup behavior.
- Improve atomic write rollback, corruption detection, migration tests and recovery UI; avoid deleting user data.
- Evaluate signed updater safely; do NOT enable unsigned auto-install/update or claim signing without actual trusted signatures and rollback.
- GitHub Actions minimal read-only token, official checked toolchain, no public tunnel or account secrets in logs. Provide clear manual Windows package verification steps.

## Workstream J — Native Windows acceptance (P0 dependency)

- If authorized physical/interactively hosted Win10 and Win11 systems are available, test all install/upgrade, notifications, DND, sleep, fullscreen/monitors/DPI/hotplug/RDP, GSMTC, WebView2 accessibility and calendars, separately by OS.
- Use scripts/phase16-native-acceptance.ps1 or a newer local-only inventory and MANUALLY recorded status. Never auto-mark PASS from code, CI or screenshots.
- Without hardware, mark NOT TESTED and give the user exact next manual actions plus unsigned CI artifacts. No fabricated native evidence.

## Batch sequencing: do more work per handoff

Batch 18.1 — A (recurrence export/source fidelity) + B (notification integrity). Ship independently tested PRs.  
Batch 18.2 — C (display/focus) + D (GSMTC). Ship PRs with explicit mock-versus-device QA.  
Batch 18.3 — E (UI/UX/accessibility) + F (productivity tools). Preserve all modules and browser tests.  
Batch 18.4 — G (system tools) + I (backups, NSIS, safe release). H only with user consent/credentials.  
Batch 18.5 — J manual Windows native acceptance if an interactive device is genuinely connected.

Keep working from one finished green PR to the next within current execution limits. If running out of context/resources, merge only independently verified green PRs and document remaining blockers in full. Batching is NOT a promise of background execution after the current chat turn.

## Mandatory final deliverables

For each merged PR: source files changed; real features; before/after tests and exact counts; latest branch head and final merged main SHA; PR URL; passing PR and main workflow URLs; checksum and verified unsigned NSIS and portable artifact URLs; browser screenshot evidence; native Win10/11 NOT TESTED status unless observed; privacy constraints and unresolved gaps.

Write cumulative docs/PHASE18_TO_22_BATCH_REPORT.md and one large docs/NEXT_ACCELERATED_BATCH_PROMPT.md including ALL outstanding workstreams and current real main/CI/artifact evidence rather than a small one-feature-only prompt.

Original REA score: 38 categories, 3 Implemented / 16 Partial / 19 Gap = 28.9% until source and native behavioral evidence justifies upgrades. Never claim 100% parity from passing CI alone.

Final objective: multiple real, verified production improvements in one execution, while keeping user data safe and enabling rapid continuation without repeated short requests.

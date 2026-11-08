> **Latest verified baseline (8 Nov 2026):** v0.24.0 PR [#27](https://github.com/wawanwfs/WFS-Notch/pull/27) merged to main `daa6b6b6a63e6b5896130056f5f6b23781d30069`, and exact-main Windows [CI 42514303079](https://github.com/wawanwfs/WFS-Notch/actions/runs/42514303079) PASS. A separate local task-due notification opt-in implementation for v0.25.0 is currently on `feat/phase24f-opt-in-task-reminders`; check PR/main CI before calling it merged or ready. After completing that workstream, prioritize the 16-module premium UI/accessibility, Notes conflict-safe persistence/structured export, recurring tasks with snooze/undo, safe installer upgrade/restoration, RFC5545 RECURRENCE-ID overrides, native display/GSMTC integration and physical Windows 10/11 acceptance; keep the complete A–J backlog below. Never claim visible Windows toast delivery from automated CI alone.

> **Verified continuation, 8 Nov 2026:** PR #26 merged into `main` as `a7b478cc95de88f4b109e550b59ebd5e00d550e8`; final-main [Windows CI 42513155235](https://github.com/wawanwfs/WFS-Notch/actions/runs/42513155235) PASS, with 64 Chromium screenshots, unsigned NSIS + portable + checksum artifacts. Workstream F local Notes tags/filter was proposed in [draft PR #27](https://github.com/wawanwfs/WFS-Notch/pull/27) for v0.24.0; verify its actual status before duplicating or merging it. Do not claim hardware acceptance from automated jobs. Next meaningful batches: opt-in task due reminders with durable ledger; Notes conflict-safe editing/folders/export; 16-module accessible UI density/contrast and responsive QA; Windows installer upgrade/restore; RECURRENCE-ID source rule fidelity; actual Windows 10/11 interactive acceptance. Keep the entire workstream backlog below active.

# WFS Notch — NEXT ACCELERATED MEGA BATCH (Phase 18 remainder through 22+)

**Execute multiple meaningful production workstreams in ONE active ChatGPT session, not just one tiny patch.** Start by fetching current real main SHA and all GitHub Actions. Do NOT pause for another user "lanjutkan" after every successful independent PR. No background-execution promise; work is performed only in the active session.

**Current verified snapshot:** v0.20.0 main bbc90ee9ef188e90a128c5a65e7d127f4fc87781, ALL PASS final run https://github.com/wawanwfs/WFS-Notch/actions/runs/42504832338 with 78 Rust tests, Clippy/rustfmt, Playwright, 64 screenshots, unsigned Windows NSIS + portable and identity audit. Workstream C v0.21.0 is being developed; check its PR and actual current main status before taking ownership of that workstream. Avoid duplicate patches.

**Already done — DO NOT rebuild unnecessarily:** PR #21 preserved source RRULE master only for untouched matching recurrence with safe flat fallback; PR #22 persisted at most 3 title-private Windows sender retries; PR #23 added editable task priorities and optional local due dates with old-state migration; v0.21.0 source includes stable unique monitor-name identity and legacy coordinate fallback when merged. Read the complete evidence in docs/PHASE18_TO_22_BATCH_REPORT.md.

**Progress benchmark:** Fixed 38-category REA reference parity remains 3 Implemented +16 Partial +19 Gap = **28.9%**, regardless of how many CI checks pass. Every completed feature must have backed source + tests; feature categories only upgrade with complete behavioral evidence. Native Windows 10/11 interactive behavior is still NOT TESTED unless someone actually provides access.

## Highest-impact next batch (execute in this order unless user supplies other priority)

**Batch 1 (two independent PRs):** Workstream A, full recurrence-master schedule reflow and RECURRENCE-ID edge cases with preview/cancel + atomic replay safeguards, THEN Workstream D GSMTC capability selection, artwork/progress stale-state regression. Ensure both are real improvements, not only documentation.

**Batch 2 (two independent PRs):** Workstream F, add opt-in task due reminders with the existing durable ledger (do not accidentally send multiple notifications on edited tasks), repeating tasks or note tagging with migration/restore tests; THEN Workstream E, accessible compact/expanded WFS visual refinement for all 16 modules, screen-reader roles, keyboard and reduced-motion tests. Preserve premium original champagne-gold dark design and existing functionality.

**Batch 3 (two independent PRs):** Workstream C deeper fullscreen/monitor/RDP safeguards and deterministic geometry/focus tests; THEN Workstream I non-destructive Windows NSIS/portable upgrade/recovery, checksum, signature-readiness and rollback. No signed release claim without a real code-signing certificate.

**Batch 4 (consent-dependent):** Workstream G Windows Wi-Fi/Bluetooth read-only status and controlled toggles; Workstream H local Ollama and external-provider integrations with securely stored opt-in credentials and actual usage metering. Skip unavailable credentials rather than ship demo integrations.

**Batch 5 (hardware-dependent):** Workstream J real interactive Win10/11 actual toast/DND, multi-monitor hotplug/RDP, GSMTC players and WebView2 accessibility. If no authorized interactive hardware is connected, explicitly mark NOT TESTED and provide local read-only manual checklist. Never infer real Windows behavior from screenshots or CI.

## Hard constraints

- Source stack Rust + Tauri 2. Start each branch from latest main after the previous verified merge; no force-push or destructive state rewrite.
- For each PR: unit, migration and deterministic DST/time/geometry tests; Clippy strict warnings, rustfmt strict; Playwright full suite including earlier Phase tests; 16×4=64 Chromium screenshots; unsigned NSIS + portable; then post-merge exact-main CI and owner identity.
- Protect user tasks, notes, local calendar source metadata, cancellation tombstones, focus timers, reminder reservations/retries, privacy settings and backwards-compatible backups. No unintended background network connection or OS settings change.
- Native hardware QA and source/browser automation must have different evidence columns. Keep unsigned binaries unsigned and avoid proprietary vendor app source/assets, copyright-infringing lyrics, credential scraping or open tunnels.
- Read actual source and baseline docs each time, not stale speculative percentages. Keep one durable cumulative report, exact PR URLs and verified GitHub artifact links.
- If a workstream cannot be safely implemented or tested, take the next independent workstream and record the blocked reason instead of stopping all progress.
- At end of active session, write another comprehensive next-batch prompt covering all unfinished workstreams with latest real main SHA and CI evidence. Do not promise continuous automatic work outside the active session.

---

## Complete workstream backlog, requirements and QA

## Workstream A — Calendar full recurrence fidelity (P0)

Phase17 implemented versioned source metadata and bounded DAILY/WEEKLY/MONTHLY/YEARLY; Phase18A additionally preserves a source RRULE master for untouched identical series with safe flat fallback. Extend these working features without destroying them.

- Preserve and extend existing source master VEVENT with UID, DTSTART TZID/UTC/floating user timezone, RRULE, EXDATE/RDATE, DTEND/DURATION, modified occurrences and cancellation tombstones. Persist fingerprints/revisions without inferring missing rules from old flattened records.
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

Batch 18.1 A/B basics were delivered in PR #21 and PR #22; continue with safe series schedule edit and richer OS permission diagnostics.  
Batch 18.2 — C (display/focus) + D (GSMTC). Ship PRs with explicit mock-versus-device QA.  
Batch 18.3 E/F should build on PR #23 task priority/due editor; implement task due reminders, notes tags, focus cycles and accessibility improvements without redoing the task editor.  
Batch 18.4 — G (system tools) + I (backups, NSIS, safe release). H only with user consent/credentials.  
Batch 18.5 — J manual Windows native acceptance if an interactive device is genuinely connected.

Keep working from one finished green PR to the next within current execution limits. If running out of context/resources, merge only independently verified green PRs and document remaining blockers in full. Batching is NOT a promise of background execution after the current chat turn.

## Mandatory final deliverables

For each merged PR: source files changed; real features; before/after tests and exact counts; latest branch head and final merged main SHA; PR URL; passing PR and main workflow URLs; checksum and verified unsigned NSIS and portable artifact URLs; browser screenshot evidence; native Win10/11 NOT TESTED status unless observed; privacy constraints and unresolved gaps.

Update cumulative docs/PHASE18_TO_22_BATCH_REPORT.md and create an equally large next-batch handoff with ALL outstanding workstreams and the verified real main/CI/artifact evidence, never just a tiny single-feature prompt.

Original REA score: 38 categories, 3 Implemented / 16 Partial / 19 Gap = 28.9% until source and native behavioral evidence justifies upgrades. Never claim 100% parity from passing CI alone.

Final objective: multiple real, verified production improvements in one execution, while keeping user data safe and enabling rapid continuation without repeated short requests.

# WFS Notch — Phase 28–36 Autonomous Multi-PR Execution Master Prompt

**Read and execute this file as a task instruction, not merely a summary.** Work through multiple safe production improvements in the current interactive session, autonomously taking the next independent workstream once its previous PR passes all gates. Do not ask the owner to type short continuation prompts after every completed feature. Do not claim work will continue asynchronously after the response ends.

**Repository:** https://github.com/wawanwfs/WFS-Notch — Rust/Tauri 2, local-first Windows 10/11, WFS original dark/champagne-gold design. Default branch main. Never force-push, reset user data, assume access to an interactive desktop or fabricate native evidence.

## Verified starting position: do not duplicate

- Source release version v0.27.0, main source commit `e3c7ed2445e857f3e9b7c4c2d6619cfebf58ac93` with [final-main Actions run #42536439574](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574) ALL FOUR JOBS PASS on 8 Nov 2026.
- **91 Rust unit tests passed**, strict Clippy and rustfmt, Playwright browser regressions, 64 headless Chromium screenshots, unsigned Windows x64 NSIS installer and portable executable, SHA256 manifest, owner identity check.
- Final artifacts: [NSIS ZIP](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574/artifacts/13639940555), [portable ZIP](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574/artifacts/13639303801), [SHA256](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574/artifacts/13639791400), [64 browser screenshots](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574/artifacts/13638928435). Links may expire.
- [PR #26](https://github.com/wawanwfs/WFS-Notch/pull/26) guarded one-time UTC/all-day series shift, NOT fully DST-aware schedule edit.
- [PR #27](https://github.com/wawanwfs/WFS-Notch/pull/27) private local Notes tags/filter.
- [PR #28](https://github.com/wawanwfs/WFS-Notch/pull/28) separately opt-in native task due reminders with durable retry/at-most-once enqueue journal.
- [PR #29](https://github.com/wawanwfs/WFS-Notch/pull/29) verified v0.26.0 persisted comfortable/compact density, local minimal motion/high contrast and safe appearance reset with Rust/browser tests; exact-main [CI #42535776621](https://github.com/wawanwfs/WFS-Notch/actions/runs/42535776621) PASS.
- [PR #30](https://github.com/wawanwfs/WFS-Notch/pull/30) verified v0.27.0 two generations of local committed state recovery, corrupt-primary/recent-backup fallback and deterministic staging failure test; exact-main CI #42536439574 PASS.
- Original fixed REA 38-category score remains **3 Implemented, 16 Partial, 19 Gap = 28.9%**. Source improvements within Partial are not grounds to upgrade entire category.
- GitHub CI is NOT interactive device QA: physical Windows 10/11 native WebView2, DND/Focus Assist, actual toast visibility, real GSMTC Spotify/Edge/Chrome/WMP playback, fullscreen, 1–3 monitor/DPI/hotplug/RDP, installer in-place upgrade and screen-reader acceptance remain **NOT TESTED**.

**Mandatory first actions:** Fetch current `main` HEAD and Actions, scan open PRs/issues, read `AGENTS.md` if present, README, CHANGELOG, `docs/feature-coverage.md`, `docs/PHASE18_TO_22_BATCH_REPORT.md`, `docs/NEXT_ACCELERATED_BATCH_PROMPT.md`, Rust/Tauri source, UI/JS/CSS and CI workflows. The SHA above is a verified baseline, not an instruction to roll the repo back if newer commits exist.

## Large execution batches — perform real changes, not docs-only placeholders

### Batch 28A — Notes conflict safety and structured local interchange (P0)

Implement optimistic edit revision / compare-and-save, conflicts during asynchronous autosave and rapid note switching, crash-safe local drafts without overwriting newer notes, controlled rollback/undo, accessible user conflict resolution and persistence across process restarts. Strictly preserve legacy note IDs, bodies and up to eight tags. Add explicit user-initiated local JSON and/or Markdown export/import with documented schema, size/count bounds, path safety, preview, deduplication and *non-destructive* transaction; no silent replacement of existing content, script execution or external network. Include corruption, repeated import and old-version backup tests, plus UI Playwright with real editor actions. Use separate PRs if large.

### Batch 28B — RFC5545 recurrence master integrity and occurrence override semantics (P0)

Start from existing ICS source rules and guarded one-time UTC/all-day shift. Add full RECURRENCE-ID single-occurrence overrides only when same UID+original DTSTART/timezone can be proven; reject unsupported RANGE=THISANDFUTURE, ambiguous TZID aliases and mixed all-day/timed cases. Keep stable UID, DTSTART, DTEND/DURATION, RRULE, EXDATE/RDATE, cancellation tombstones, reminders and modified IDs. Preview exact affected old/new dates, duration, collision count, DST gap/overlap and reminder risks before atomic scoped edits. Preserve one master plus valid exceptions on export ONLY when a fresh reimport would round-trip; otherwise flatten safely without duplicating occurrence IDs. Test Feb29, Jan31, negative BYMONTHDAY, BYDAY ordinal, WKST, COUNT/UNTIL, 250-event limits, DST IANA transition, repeated import and restore.

### Batch 29 — Productivity expansion without irreversible mutations (P1)

Tasks: bounded recurrence daily/weekly, clear completed with undo/confirmation, due snooze and durable reminder suppression after edit. Preserve titles, priorities, due times, existing opt-in flags and private retry ledger. Pomodoro: persisted work/break cycle, pause/resume/reboot/sleep/clock-change semantics, no unconsented OS toasts. Notes: folders/sort only after conflict-safe core above. Health: optional quiet hours, daily rollover and opt-in reminders; no medical claims. Test deterministic clock injection, migrations, no dup alerts, keyboard screen-reader labels and full backup round-trip.

### Batch 30 — Full native display, fullscreen and focus guarantees (P0)

Extend stable device-name monitor targeting without falling onto unrelated screens; persist preference across primary swap, 1–3 monitors, negative origin and unplug/replug; no focus stealing or global hooks. Test geometry at 100/125/150% mixed DPI, resolution changes, maximized versus exclusive/borderless fullscreen and RDP reconnect. Add deterministic Rust mock/topology tests and browser layout tests; mark real OS compositor results NOT TESTED until interactive proof. Do not treat headless screenshots as 3-monitor evidence.

### Batch 31 — GSMTC correctness with native capability checks (P1)

Only target a uniquely resolvable Windows media session. Preserve existing duplicate browser AUMID fail-closed guard and stale artwork invalidation; implement safe per-instance resolution if Windows APIs expose a stable handle, otherwise remain disabled. Validate playback/seek capability, progress drift, sleep/resume, missing album art, oversized image data, disconnects and rapidly switching players. Test Rust fake sessions, JS asynchronous races and native manual Spotify/Edge/Chrome/Windows Media Player checks separately. No copyrighted lyric scraping or unlicensed playback assets.

### Batch 32 — Premium original WFS UI/UX and accessibility (P1)

Audit **ALL 16 modules**, collapsed/expanded navigation, 760x470 and smaller logical viewport, responsive scroll, premium original dark/champagne-gold glass design and accessible contrast. Build on the new persisted density, high contrast and minimal motion; do not re-add them. Improve functional empty/loading/error states, hierarchy, keyboard focus order, screen-reader announcements, touch/keyboard hit targets, reduced-motion and dark theme consistency. Keep controls functional, not decorative. Add Playwright covering each module + 64 screenshot baseline; no proprietary vendor assets or copied competitor code.

### Batch 33 — Installer, backup, release and integrity (P1)

Build on the committed two-generation state rotation, do not discard `state.json.bak.prev` as unknown garbage. Test simulated interruption during backup/primary rotation, recovery if primary and latest backup both unreadable, oversized state and unsafe old migrations, and retention of notification journals and ICS tombstones. Provide local read-only SHA256 verification, Windows installer upgrade from old app versions and rollback runbook. No signed auto-updater claim without certificate or user consent. Installer/portable CI artifacts are unsigned and do not verify physical upgrade; record native Win10/11 observations as separate evidence.

### Batch 34 — Safe read-only Windows telemetry and optional OS controls (P2)

Expose truthful read-only Wi-Fi/Bluetooth presence/status with stable Windows APIs and error/capability states. CPU/RAM/disk/network/GPU/thermal only where accessible without invasive drivers; bounded polling. System mutation toggles require explicit per-action permission and no admin escalation. Clipboard/shelf/file actions require scoped consent, sensitive data redaction and safe preview; no global surveillance, user identity scan or open remote tunnel.

### Batch 35 — Optional AI/provider integrations (consent and dependencies, P2)

Local Ollama first if an explicitly installed/allowed service exists. Cloud AI/Claude/GitHub capabilities only via real official provider APIs, user-authorized least-privilege scopes, Windows DPAPI/Credential Manager tokens, revocation UI, honest error/usage reporting and actual provider-side cost units; no synthetic token meter. Skip credentials-dependent steps when unavailable instead of blocking unrelated work. Weather/subscriptions need opt-in privacy. Do not deploy uncontrolled background network services.

### Batch 36 — Native Windows acceptance / release decision (external dependency, P0)

On authorized interactive Windows 10 and 11 devices, validate install/upgrade/uninstall/portable state, toast/DND/lock/sleep, media sessions, fullscreen/hotplug/multi-DPI/RDP, focus, keyboard and screen reader. Record observed PASS/FAIL and device metadata using `scripts/phase16-native-acceptance.ps1`; sanitize evidence locally, never upload private notes/screenshots or claim hardware coverage from CI. If no device is connected, mark EVERY such acceptance NOT TESTED and provide exact user-run steps.

## Universal engineering constraints and gate

1. Make one independently testable source workstream per PR; batch multiple successive PRs in one active session when possible. Always create the feature branch from latest verified main, compare previous implementations, and preserve local data and backwards-compatible state. No force pushes, deletions of working features, unapproved network or invasive OS changes.
2. Rust/Tauri 2 remains the stack; use stable official APIs, bounded inputs and meaningful error reporting. Never silently reinterpret legacy calendar recurrence IDs, backup snapshots, note data or notification reservations.
3. CI on the **exact PR head** must pass Windows MSVC `cargo test --all-targets --verbose`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, full Playwright and historical Phase tests, 16 modules × 4 screenshot scenarios = 64, unsigned NSIS+portable and SHA256 evidence.
4. Fix failed CI and re-run before review. Check mergeability and exact expected PR SHA; squash or merge only green PRs. After merge require a **separate exact-final-main successful CI**, identity job success and artifact links/hashes. Never report pending as success.
5. Keep a cumulative Markdown audit/report with each real source change, regressions and test count, CI and PR URLs, final main SHAs and artifact links, privacy findings and native QA columns. No unverifiable parity percentage increases; retain 28.9% until full category proof.
6. Cross-version migrations and backups must preserve source calendar metadata, cancellation tombstones, task reminder opt-in and delivery/retry journals, notes tags, app settings, local user files and original backups. Test old JSON migration and repeated import idempotency.
7. When credentials, licenses, hardware, OS permission or reference assets are missing, skip that dependent part and implement the next independent workstream with blockers documented. Use official third-party APIs and do not lift copyrighted assets, synced lyrics, icons or layouts.
8. Each finished session must leave a **comprehensive next-batch Markdown prompt** in docs that identifies current real main SHA, exact CI, artifacts, completed versus blocked workstreams and next multiple priorities. Avoid short one-feature handoffs. All background-monitoring claims require an explicitly configured automation; otherwise all work ends with the current response.

## Mandatory final delivery format

Report versions/commits, PR links, what changed, 91+ updated exact Rust test count from successful workflows, browser screenshot and Windows package links, SHA256 manifest, real Windows manual status, old and new reference parity only when justified, security caveats and outstanding top 5 batches. Avoid claiming 100% completion from green automated builds. Present clear Windows PowerShell local verification steps when user needs to install/test artifacts.

**Execution begins by fetching live HEAD, not by asking for confirmation.**

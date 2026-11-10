# WFS Notch — Phase 18–22 Accelerated Batch Progress

**Repository:** https://github.com/wawanwfs/WFS-Notch

**Scope:** rolling report of completed production PRs; this does NOT assert that Phases 18–22 are finished. Phase 18 started from v0.17.0, main 3215454f155d742b9656f381e458cb2f7bceb2e8.

## Verified merges

| Workstream | PR | Final main SHA | Final main CI | Verified change |
| --- | --- | --- | --- | --- |
| A: Safe calendar ICS export | [#21](https://github.com/wawanwfs/WFS-Notch/pull/21) | 28e56737c10b4fe2116cc22bead2b2847f6a558d | [42501487188](https://github.com/wawanwfs/WFS-Notch/actions/runs/42501487188) PASS | Source RRULE master only when strict re-expansion matches every event; edited/canceled/legacy series flatten safely; 73 Rust tests |
| B: Native reminder backoff | [#22](https://github.com/wawanwfs/WFS-Notch/pull/22) | 8ba8956e7ffb1b5f4c434a019aad7a3896c97ae4 | [42502645629](https://github.com/wawanwfs/WFS-Notch/actions/runs/42502645629) PASS | 3-attempt persisted sender retry with 20/60/180-sec backoff, backup/restart safety and title-free generic errors; 76 Rust tests |
| F: Task edit, priority, due time | [#23](https://github.com/wawanwfs/WFS-Notch/pull/23) | bbc90ee9ef188e90a128c5a65e7d127f4fc87781 | [42504832338](https://github.com/wawanwfs/WFS-Notch/actions/runs/42504832338) PASS | Local task editing, low/normal/high priority, optional due time, stable ID/completion, old JSON compatibility and browser regression; 78 Rust tests |

All three final-main workflows passed Windows MSVC Rust tests, strict Clippy and rustfmt, full browser suites, 64 headless Chromium screenshots, unsigned NSIS/portable and owner-identity audit.

## Latest completely verified Windows package before Workstream C

Source version **v0.20.0**, SHA bbc90ee9ef188e90a128c5a65e7d127f4fc87781.
Final run https://github.com/wawanwfs/WFS-Notch/actions/runs/42504832338

- Installer: https://github.com/wawanwfs/WFS-Notch/actions/runs/42504832338/artifacts/13626649773
- Portable: https://github.com/wawanwfs/WFS-Notch/actions/runs/42504832338/artifacts/13626699445
- SHA256SUMS: https://github.com/wawanwfs/WFS-Notch/actions/runs/42504832338/artifacts/13626689453
- Browser screenshots: https://github.com/wawanwfs/WFS-Notch/actions/runs/42504832338/artifacts/13627440478
- SHA256 installer WFS Notch_0.20.0_x64-setup.exe: c1471ea983f2f02f5d69e778409d63e73096195bc50c559c269348ceab55d220

These unsigned artifact ZIP URLs may expire according to GitHub retention.

## Workstream C — Stable saved monitor identity (implementation in progress)

Feature branch feat/phase18c-stable-monitor-identity targets source version v0.21.0.

- A unique bounded Windows/Tauri monitor device name becomes the saved preference; this remains stable when monitor coordinates and DPI change **if** the driver continues to report that same unique name.
- Unnamed, duplicate or unsafe device names use legacy origin:X:Y identifiers. Previously saved origin preferences keep resolving to their exact coordinates on upgrade; they are never silently rewritten.
- A disconnected saved display falls back to current primary while retaining original selection for eventual reconnection. UI distinguishes a connected legacy origin preference from an unavailable selected display.
- Monitor topology polling also notices device-name changes. Tests cover stable names after rearrangement, signed virtual origins, duplicate/unsafe names and preference validation.
- Its final release status must be established from latest PR-head and final-main CI; this report is not a release PASS claim for v0.21.0.

## Workstream D — merged and independently verified (v0.22.0)

PR [#25](https://github.com/wawanwfs/WFS-Notch/pull/25) merged to main SHA `9acc91d841e1640596c2259926790b204ff9d5e4`. Exact final-main [GitHub Actions run 42510653325](https://github.com/wawanwfs/WFS-Notch/actions/runs/42510653325) **ALL FOUR JOBS PASS**: 81 Rust tests, Clippy, rustfmt, browser suites including new mocked GSMTC duplicate-ID regression, 64 headless screenshots, unsigned NSIS/portable and owner identity audit.

- [Installer ZIP](https://github.com/wawanwfs/WFS-Notch/actions/runs/42510653325/artifacts/13629900118)
- [Portable ZIP](https://github.com/wawanwfs/WFS-Notch/actions/runs/42510653325/artifacts/13629361938)
- [SHA256 ZIP](https://github.com/wawanwfs/WFS-Notch/actions/runs/42510653325/artifacts/13629272362)
- [Browser screenshot ZIP](https://github.com/wawanwfs/WFS-Notch/actions/runs/42510653325/artifacts/13628549172)
- SHA256 `WFS Notch_0.22.0_x64-setup.exe`: `93b9a9ea336551deccc80a5e0fa1655cbaaf3e0192e02277d0578fb70ac5c131`

GSMTC now refuses arbitrary control of browser tabs sharing a SourceAppUserModelId, invalidates stale artwork, guards asynchronous player selection, and bounds progress projection after sleep/resume. This is a safety fix, not full per-tab selection. Physical Win10/11 interactive media QA remains NOT TESTED.

## Workstream A — guarded one-time series shift in v0.23.0 candidate

The feature branch `feat/phase22a-safe-series-day-shift` implements preview/freshness-guarded atomic whole-day moves of existing *untouched* date-only or UTC recurring instances, preserving event IDs, reminder history and deletion tombstones, and marking all moved instances individually edited to ensure source-master RRULE export safely falls back to independent events. It rejects TZID/floating DST and previously edited source series. Real recurrence-master editing, multi-shift support and RECURRENCE-ID are **not** implemented.

**Verification status:** this section is a source scope description ONLY until the exact v0.23.0 PR-head and post-merge main CI are green. Do not call it merged or release-ready before that evidence exists.

## Exact remaining issues by workstream

A: Full recurrence source schedule changes with preview/rollback; RECURRENCE-ID override and exception semantics; native consented ICS open/save; optional subscriptions only with explicit credentials/permission.
B: Validate real Windows toast permission, privacy, DND/Focus Assist, sleep, lock screen and queue behavior on interactive Windows 10 and 11. At-most-once sender enqueue and bounded error retry cannot prove exactly-once *visible* notification.
C: Physically test 1–3 monitors, primary swaps, hotplug, RDP, mixed 100/125/150% DPI, fullscreen versus maximized and focus stealing. Device names can change across drivers and reconnects.
D: GSMTC source selection/artwork invalidation/progress drift and real Spotify, supported browser, Windows Media Player session checks.
E: Premium original WFS dark/gold UI audit across all 16 modules, screen reader, keyboard, contrast, reduced motion and responsive layout.
F: Opt-in task due reminders, recurring tasks, snooze/undo, notes tags and folders, work-break Pomodoro cycles and wellness preferences. Due dates do not yet trigger an OS notification.
G: Read-only official Wi-Fi/Bluetooth status with explicit consent for any system modification; safer dock/shelf/tools.
H: Optional local Ollama and provider AI only with explicit user consent, OS secure credential storage and actual provider usage; never simulate integrations.
I: Native installer upgrade/data restoration and signed update path; binaries are currently UNSIGNED.
J: Actual interactive Windows 10/11 native verification and sanitized on-device evidence; currently NOT TESTED.

## Privacy and QA limits

No attached Windows GUI hardware is available for true on-device acceptance. GitHub Actions Windows builds and headless Chromium evidence do **not** test physical notifications/DND, multi-monitor hotplug/RDP, real GSMTC player behavior, or native WebView2 accessibility. The local-only scripts/phase16-native-acceptance.ps1 records user-observed PASS/FAIL separately.

No proprietary third-party app source/assets or unlicensed lyrics were copied. No public tunnel, process-title tracking, credential scraping or unsigned auto-updater was added.

## REA weighted parity

**38 fixed categories: 3 Implemented, 16 Partial, 19 Gap = 28.9%.** Calendar, Notifications, Todo and compact/expanded monitor behavior remain Partial. Internal workstream progress does not automatically advance reference parity.

Continue with [NEXT_ACCELERATED_BATCH_PROMPT.md](NEXT_ACCELERATED_BATCH_PROMPT.md), which groups the remaining work into multiple PRs without requiring one-sentence repeated continuations. Each PR must pass exact-current-head CI and final-main verification.

## Workstream A — verified merged v0.23.0

PR [#26](https://github.com/wawanwfs/WFS-Notch/pull/26) merged into main `a7b478cc95de88f4b109e550b59ebd5e00d550e8`. Exact-main [CI 42513155235](https://github.com/wawanwfs/WFS-Notch/actions/runs/42513155235) **PASS**. Artifacts: [unsigned NSIS](https://github.com/wawanwfs/WFS-Notch/actions/runs/42513155235/artifacts/13630032749), [unsigned portable](https://github.com/wawanwfs/WFS-Notch/actions/runs/42513155235/artifacts/13630261632), [SHA256SUMS](https://github.com/wawanwfs/WFS-Notch/actions/runs/42513155235/artifacts/13630052625), [64 screenshots](https://github.com/wawanwfs/WFS-Notch/actions/runs/42513155235/artifacts/13630900060).

The only schedule move implemented is a guarded atomic *one-time* whole-day shift of untouched UTC/all-day local recurrence instances. Full RECURRENCE-ID and DST-aware recurrence master edits remain not implemented. Interactive native Windows 10/11 remains NOT TESTED.

## Workstream F — private Notes tags v0.24.0 PR #27 candidate

Draft [PR #27](https://github.com/wawanwfs/WFS-Notch/pull/27) adds bounded persisted tags, old JSON migration, tag filter and tag-aware search, and Rust/browser tests. **The PR is not merged or release-ready until PR-head Windows CI and exact post-merge main CI pass.** All Notes remain stored only on the device, with text-only labels and no network indexing.

Do not mark Notes fully implemented: folders, rich text, robust conflict handling, undo and structured export/import remain incomplete. REA weighted 38-category parity still 3 Implemented / 16 Partial / 19 Gap = 28.9%.

## Workstream F — verified merged v0.24.0 Notes

PR [#27](https://github.com/wawanwfs/WFS-Notch/pull/27) was merged into main at `daa6b6b6a63e6b5896130056f5f6b23781d30069`. Exact postmerge [GitHub Actions 42514303079](https://github.com/wawanwfs/WFS-Notch/actions/runs/42514303079) **PASS**. All four CI job statuses (3 active passes and an expected identity gate skip) are distinct from native Win10/11 acceptance, which is still NOT TESTED. 85 Rust tests passed on the v0.24.0 PR-head; tag search/filter and browser regressions passed. Notes stays Partial.

## Workstream F — v0.25.0 task reminders candidate

On `feat/phase24f-opt-in-task-reminders`, implement task due alerts as a separate opt-in, reusing the 20-second native Windows scheduler and durable at-most-once enqueue + bounded sender retry. Only incomplete, currently due tasks in a 5-minute grace period qualify; task IDs/due timestamps are namespaced and private titles redacted by default. Browser never fakes OS delivery. Unit tests cover opt-in, edits, completions, restarts and retry. **Status: candidate only** until exact PR-head CI and final main CI independently PASS; no real native Windows 10/11 acceptance has been performed.

## Phase26E — accessibility appearance settings PR candidate

Baseline verified main v0.25.0 `9ba9c9e37361ec84b934eda564cb881bb516cf33`, exact postmerge [CI 42515389548](https://github.com/wawanwfs/WFS-Notch/actions/runs/42515389548) ALL PASS, including 88 Rust tests, Chromium/64 screenshots, unsigned Windows NSIS+portable, SHA256 and commit identity.

Feature branch `feat/phase26e-accessible-appearance-presets` implements persisted compact/comfortable spacing, independent reduced motion, high contrast, reset and test coverage. **Candidate only until its exact PR-head and post-merge main GitHub Actions are green.** Device Win10/11 UI/keyboard/screen-reader behavior remains NOT TESTED. Fixed 38-category parity stays 28.9%.

## Phase26E — exact verified merged UI settings (v0.26.0)

PR [#29](https://github.com/wawanwfs/WFS-Notch/pull/29) merged to main `077859e1b3eb72ef3c5e5a06c0823ee4f755bb06` after passing PR-head [CI 42535562012](https://github.com/wawanwfs/WFS-Notch/actions/runs/42535562012). The post-merge main run is [42535776621](https://github.com/wawanwfs/WFS-Notch/actions/runs/42535776621); check this run's result rather than assuming PASS from PR CI. Features: compact/comfortable density, minimal motion, optional high contrast and reset with persisted Rust migration tests. Native Windows GUI acceptance remains NOT TESTED.

## Phase27I — robust local state recovery PR candidate

Feature branch `feat/phase27i-state-recovery-retention` adds two retained local recovery generations, guarded fallback and deterministic forced-staging-error rollback tests. **Candidate only** until its own Windows CI and exact-main CI pass. Not a claim that native NSIS upgrades from older versions have been tested.

## Phase 26E + Phase 27I — independently verified final main, 8 Nov 2026

| Workstream | Pull request | Exact merged main SHA | Final main GitHub Actions | Source and test evidence |
| --- | --- | --- | --- | --- |
| E — persisted visual accessibility preferences | [#29](https://github.com/wawanwfs/WFS-Notch/pull/29) | `077859e1b3eb72ef3c5e5a06c0823ee4f755bb06` | [42535776621](https://github.com/wawanwfs/WFS-Notch/actions/runs/42535776621) PASS | Rust persistence/migration, compact/comfortable density, opt-in high contrast and local motion suppression, safe reset, browser UI; **89 Rust tests** |
| I — local crash-recovery retention | [#30](https://github.com/wawanwfs/WFS-Notch/pull/30) | `e3c7ed2445e857f3e9b7c4c2d6619cfebf58ac93` | [42536439574](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574) PASS | Two generations `state.json.bak` and `state.json.bak.prev`, corrupt primary+latest recovery and deterministic staging failure regression; **91 Rust tests** |

**Exact final-main v0.27.0 artifact evidence:** [unsigned NSIS](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574/artifacts/13639940555), [unsigned portable](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574/artifacts/13639303801), [installer SHA256 manifest](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574/artifacts/13639791400), [browser screenshot evidence](https://github.com/wawanwfs/WFS-Notch/actions/runs/42536439574/artifacts/13638928435). Artifacts may expire per GitHub retention. GitHub Actions runner test != real local Win10/11 acceptance. No signed updater, Windows installation/upgrade acceptance, real GSMTC media player, toast visibility, mixed DPI monitor/hotplug/RDP or native screen reader QA has been performed. Keep all explicitly **NOT TESTED**.

Do not count these as two additional reference categories completed: fixed 38-category parity remains 3 Implemented, 16 Partial and 19 Gap (**28.9%**). Proceed to [NEXT_PHASE28_MASTER_PROMPT.md](NEXT_PHASE28_MASTER_PROMPT.md) for the next accelerated multi-PR workstreams.

## Phase 28A1 — Notes optimistic concurrency PR candidate

Baseline v0.27.0 latest green main `f51ff8ef892acde476a60273998426a73c67a90a`, [CI 42537043664](https://github.com/wawanwfs/WFS-Notch/actions/runs/42537043664) all four jobs PASS. Workstream Phase28A1 branch `feat/phase28a-note-revisions-conflict-recovery`: native Notes revision CAS, explicit conflict actions, local draft restoration, stale-delete protection, tests. PR/main checks must pass before claiming completion. Notes JSON export/import not yet included; follow in separate PR. Native Win10/11 remains NOT TESTED and reference parity fixed 28.9%.

## Phase28A1 — exact verified code baseline

PR [#32](https://github.com/wawanwfs/WFS-Notch/pull/32) merged into main `266f27e6e80df3f5cda96bdaaa780b5e29fcc921`, exact-main [CI 42543253059](https://github.com/wawanwfs/WFS-Notch/actions/runs/42543253059) all four PASS (plus independent PR-head green run). Notes revision CAS and local draft conflicts shipped as v0.28.0. Native Windows GUI NOT TESTED.

## Phase28A2 — bounded local JSON interchange PR candidate

Branch `feat/phase28a-notes-json-interchange` adds native JSON export/preview/append-only import and private user-confirmed copy/paste UI. Verify exact PR and main workflow before marking shipped. Preview does not mutate disk; duplicate IDs are skipped without replacing content. No path permissions, uploads or cloud sync.

## Phase28A3 — backup-restore revision safety PR candidate

Source baseline v0.29.0 `408744a28e2886b5dbc7bade957a52a48a6e7a5d`, exact-main [CI 42544852146](https://github.com/wawanwfs/WFS-Notch/actions/runs/42544852146) ALL FOUR PASS (93 Rust tests). Branch `fix/phase28a-notes-restore-revisions` prevents backed-up Notes from resetting optimistic revisions for overlapping note IDs and introduces focused Rust+browser regressions. Validate exact head and post-merge main CI before declaring this safe to ship. No interactive Win10/11 acceptance evidence; fixed REA remains 28.9%.

## Phase 28B1 — scoped RECURRENCE-ID source support (candidate)

Baseline v0.30.0 `e9647bd2fd647999a1931268c11552df073031a5`, [CI 42546126393](https://github.com/wawanwfs/WFS-Notch/actions/runs/42546126393) all four PASS, 94 Rust tests, unsigned Windows NSIS and portable. Candidate `feat/phase28b-rfc5545-occurrence-overrides` implements explicit same-UID original-occurrence overrides with fail-closed DST/RANGE/cancellation and verified source-export roundtrip; PR checks and post-merge exact-main CI must be verified before marked shipped. Native Win10/11 remains NOT TESTED; reference parity 28.9%.

## Phase30A — explicit monitor disconnect placement (candidate)

Verified v0.31.0 latest main `c61aea253c702de838a6c63ec6a16b89fce26e2f`, [exact-main CI 42726846909](https://github.com/wawanwfs/WFS-Notch/actions/runs/42726846909) ALL FOUR PASS, 97 Rust tests, unsigned NSIS+portable+SHA256. Branch `fix/phase30-monitor-disconnect-no-fallback` avoids moving a specifically pinned but unavailable monitor to primary; disabled topmost while missing; restored on topology change; explicit native status and regression. PR-head and post-merge CI must be verified. Physical monitor, DPI, RDP acceptance still NOT TESTED. Reference parity remains 28.9%.

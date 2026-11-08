# WFS Notch Phase 12 — Native Windows Reliability & Feature Expansion

## Provenance
- Baseline: v0.11.0, `main`, commit `c30d395a3fda403e9050f0e7cec55244d2ac671c`.
- Work branch: `feat/phase12-native-reliability`; pull request: https://github.com/wawanwfs/WFS-Notch/pull/14.
- Rust + Tauri 2, original WFS brand/assets and project structure retained. No reference vendor source/assets copied.
- Target app version: **v0.12.0** (release only if all required CI checks pass).

## Source modifications

| Workstream | Implementation in Phase 12 | Remaining qualification |
| --- | --- | --- |
| Reminder reliability | Existing Rust store now persists a title-free delivery key/expiry in the same fsync+rename state transaction before Windows toast enqueue; retries transient OS errors; catches up within 5 minutes after event time; edited events generate a fresh key; deterministic clock/restart tests | At-most-once enqueue attempts, **not** exactly-once OS display. Crash between reservation and API call may miss one toast. A Windows acceptance response is not a visible-delivery receipt. No system-wide notification interception. |
| Windows toast settings | Read-only WinRT `ToastNotifier.Setting` diagnostic with disabled/app/policy states, unknown fallback. Does not claim the desktop Tauri plugin's always-granted state proves system delivery. | Windows Focus Assist, lock-screen suppression and portable/no-shortcut behavior still require physical tests. |
| Fullscreen / multi-monitor | Opt-in foreground fullscreen detection via Win32 window rect and selected monitor; lowers topmost without calling focus; guard repositions on monitor geometry/scale/topology changes. | Heuristic can miss borderless/game-exclusive windows, virtual desktops and other non-standard surfaces; native DPI and hotplug QA required. |
| GSMTC media | Fetch by selected app model ID, normalize nonzero timelines, suppress unsupported playback actions, recheck capability before invoking and gracefully show disconnected sessions. | OS-side session changes, artwork handling and cross-player behavior not yet verified on Windows devices. No copyrighted or scraped lyrics introduced. |
| UI/UX | Small viewport overflow guards, keyboard focus outlines retained, compact/expanded transition respecting reduced motion, settings for fullscreen and notification status. | Pixel matching to an external app is not claimed. |
| Browser evidence | 16-module screenshot matrix at 100%, 125%, 150% simulated DPR plus 560×420 small viewport, 64 images and horizontal overflow assertions. | Headless browser screenshots are not native Tauri/Windows screenshots, and no golden-image pixel baseline is asserted. |

## 38-category weighted progress
Reference: `docs/feature-coverage.md`. Implemented **3**, Partial **16**, Gap **19**.

`(3*1 + 16*0.5 + 19*0)/38 * 100 = 28.9%` rounded to one decimal.

**Progress remains 28.9%:** meaningful improvements were made inside several Partial categories, but those categories still have reference requirements or Windows behavioral QA gaps. Do not inflate parity.

## Quality gates: evidence policy

- Source CI: `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, Playwright browser tests and screenshot evidence.
- Windows packaging: GitHub Actions Windows 2022 Rust MSVC and unsigned Tauri/NSIS bundle; upload SHA256 and portable executable as workflow artifacts.
- The GitHub Actions browser and Windows build runs **do not represent physical interactive Windows testing**.
- Separate manual Windows QA **not performed by this automation**: real Windows 10 and 11 hardware, 1–3 monitors, DPI 100%/125%/150%, resolution switch and hot-plug, app fullscreen foreground + click-through/focus, toast disabled, sleep/resume + clock skew, Spotify/browser/Windows Media Player artwork/playback.
- Before any merge, inspect the *latest PR head* and every required job conclusion. A green older run or cancelled superseded run is insufficient.

## Open risk / remediation
- Exactly-once visible Windows toast delivery is not achievable via a local reservation + platform enqueue API. Consider deterministic delivery IDs and an OS notification history reconciliation strategy if a supported API is available with the correct app identity; never duplicate on restart merely to increase reported delivery counts.
- Display detection uses a simple foreground rectangle check, not exclusive-fullscreen or anticheat interaction. Preserve opt-out.
- Media session IDs are app-model IDs, which may not uniquely identify multiple instances; do not claim per-instance routing.
- Lyrics remains a gap until a legally licensed or user-owned source and its terms are validated.
- REA/Craft Anything/LeanREA/Ghidra were not used to extract vendor assets or proprietary code; no authorized binary/reference asset was supplied in this phase.

## Merge criterion
PR is intentionally opened as draft. Merge only when **ALL** PR-head Rust, Clippy, rustfmt, browser regression/screenshot and Windows NSIS jobs report success; otherwise leave unmerged and report the failing job with links.

# WFS Notch — Phase 14 Full Execution Prompt

Continue development of [WFS Notch](https://github.com/wawanwfs/WFS-Notch) from the **actual latest `main`**, not a remembered SHA. Read `README.md`, `docs/feature-coverage.md`, `docs/PHASE12_REPORT.md`, `docs/PHASE13_REPORT.md`, `docs/security.md`, all production Rust modules, frontend files and recent GitHub Actions/PRs. Preserve the original WFS brand, modern dark/champagne-gold style, Rust + Tauri 2, local-first privacy and existing features. Do not access/copy proprietary Poni source or assets; do not deploy public Cloudflare tunnels or remote shells.

## P0: Actual Windows validation

1. Build from a clean Windows 10/11 environment (or an authorized interactive Windows machine) using Rust/MSVC, Windows SDK and WebView2. Validate unsigned NSIS installation and portable execution separately. Capture reproducible **local** diagnostic logs and screenshots without exposing private notifications, filenames or user data. Treat CI packaging as a separate check.
2. Use physical or attached test displays in 1-, 2- and 3-monitor configurations; DPI 100/125/150%; primary and secondary fullscreen (exclusive/borderless/maximized), negative virtual origins, resolution changes, hotplug/unplug and RDP reconnect. Verify no focus steal or unsafe input interception, and keep the opt-in policy.
3. Validate Windows notification permissions and suppression (per-app settings, OS disabled, Focus Assist/DND, lock screen, sleep/resume, edited/cancelled reminder, startup and time rollback). Prove what OS APIs actually acknowledge. Do not claim exactly-once *visible* delivery when Windows provides no such receipt.
4. Validate GSMTC per-source controls with Spotify, supported browsers and Windows Media Player. Record what happens when a session disconnects or control is disabled. SourceAppUserModelId alone is not an unambiguous session-instance identifier; add a better identifier only if the Windows API genuinely provides one.

## P1: Highest-value source improvements

5. Add reliable recurrence and TZID/floating-date parsing to calendar with controlled date/time tests and no silent timezone shifts. Introduce safe idempotent migrations; do not regress the source UTC event model.
6. Continue deterministic backend test seams: corrupted state fallback, platform failure and backoff/retry, clock jumps, backup restore, duplicate startup and synchronization; do not persist notification titles in the delivery journal. Evaluate a supported native acknowledgement strategy before modifying delivery semantics.
7. Improve monitor placement and progress/media state tracking without global input hooks or unsupported WinRT assumptions.
8. Add a real Playwright golden image comparison **only after approved WFS-owned reference screenshots are present**. Distinguish headless Chromium pixel diffs from native WebView2 screenshots. Keep test failures inspectable with per-module artifacts, capture at supported DPI.
9. Develop new features gradually via isolated modules: opt-in user-owned local lyrics, consent-driven clipboard, device status/controls where Windows APIs permit, and accessibility polish. Validate rights, permissions and privacy. Never scrape or bundle copyrighted lyrics without authorization.

## Quality and release requirements

- Create `feat/phase14-...` from updated `main`, commit real Rust/JS/CSS functionality, tests and documentation, open a PR on the connected GitHub account. No force-push.
- Run `cargo test --all-targets --verbose`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, Node syntax, all Playwright tests, multi-DPI screenshot test and Windows NSIS + portable build.
- Keep **Windows interactive QA and browser UI QA as separate reports**. If Windows interactive hardware isn't available, explicitly state `NOT TESTED`.
- Merge only if every required CI job for the *exact latest head SHA* is green; review main push CI including owner identity gate afterward. Never assert an unsigned installer has a signature.
- Maintain the exact original 38-category matrix: Implemented=1, Partial=0.5, Gap=0; show the counts and `(I + 0.5 P) / 38 × 100`, no unjustified promotions.
- Deliver updated source main SHA, PR and workflow links, unsigned installer and portable artifact URLs if build passes, Windows device test notes, UI screenshots, `docs/PHASE14_REPORT.md` and `docs/NEXT_PHASE15_PROMPT.md`. Do not claim 100% parity without independent reference observation and on-device proof.

**Priority rule:** A truthful partially completed phase with green automated gates and pending native QA is preferable to claiming physical Windows readiness without tests.

# WFS Notch — Phase 13 Complete Implementation Prompt

**Repository:** https://github.com/wawanwfs/WFS-Notch  
**Base:** Use the actual latest `main` HEAD. If Phase 12 PR https://github.com/wawanwfs/WFS-Notch/pull/14 is not merged, audit it and its failures before continuing; never assume a version tag proves CI or native validation.

**Mission:** Continue the original WFS Notch Rust + Tauri 2 Windows 10/11 project. Preserve the premium WFS dark/champagne-gold design, branding, local-first privacy, original module schema, migration/backup behavior, accessibility and working features. No vendor source/assets, no unlicensed lyrics, no privileged hooks or public Cloudflare Tunnel.

## Mandatory read and audit
1. Read `README.md`, `docs/feature-coverage.md`, `docs/PHASE12_REPORT.md`, `docs/architecture.md`, `docs/security.md`, latest GitHub Actions and any existing PRs before writing code.
2. Establish actual baseline SHA, source paths, CI workflow and artifacts. Audit `src-tauri/src/notifications.rs`, `display.rs`, `media_sessions.rs`, `main.rs`, `src/features/media.js`, `src/app.js`, `tests/browser_screenshot_matrix.py`.
3. Use REA / Craft Anything / LeanREA / Ghidra **only** if tools are installed and the user has supplied a lawfully usable reference. Never copy proprietary Poni assets, source or private implementation.

## P0 — Prove native reliability
- On physical Windows 10 and Windows 11 (or an approved interactive machine), test native toasts with installed NSIS app and with portable build separately. Check disabled per-app and global notifications, Focus Assist / DND and lock-screen privacy without requesting access to third-party notifications.
- Build a deterministic time-injected scheduler harness to test timezone, DST, sleep/resume, wall-clock jumps, edit/cancel event, near-boundary reminders, process interruption, JSON backup/restore, permission revocation, API rejection and repeated startup. Document at-most-once enqueue semantics and missing display receipts. Do not falsely claim exactly-once delivery.
- Validate and improve Win32 fullscreen detection including borderless apps, multiple monitors (1–3), DPI 100/125/150, negative monitor coordinates, hotplug and RDP reconnect. Do not steal focus, interfere with games' input/anticheat, or blanket-hook system windows; keep opt-in and manual override.
- Verify WinRT GSMTC player enumeration, changing tracks, seek/progress, artwork limits, unsupported buttons and disconnect/reconnect for Spotify/browser/Windows Media Player. Where SourceAppUserModelId cannot uniquely identify sessions, provide stable session selection only if Windows API supports it.

## P1 — Product quality
- Add genuine Playwright screenshot **golden comparisons** with approved WFS-owned reference screenshots for all 16 modules, three DPIs and small viewports. Keep artifact evidence for failures, distinguish headless Chromium from native WebView2.
- Improve compact/expanded transitions, reduced-motion handling, keyboard semantics, mouse hover/focus management, contrast, touch target sizing, dynamic layout, high-DPI artwork, settings discoverability and privacy copy.
- Introduce lyrics only with user-uploaded/user-owned text or an approved licensed source. Check rights, rate limits, attribution and cache licensing; preserve clear opt-in. Do not scrape protected lyrics.
- Continue highest-value missing feature categories (scheduler recurrence/TZID, opt-in clipboard, privacy-aware local utilities, device controls) with separate scoped PRs and objective acceptance tests.

## Required implementation and quality workflow
1. Create `feat/phase13-...` from up-to-date `main`, do not rewrite existing working modules.
2. Implement production Rust/JS/CSS source, unit/integration tests and migration logic, not just README/progress notes.
3. Run `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, browser Playwright suite, screenshot regression and Windows NSIS/portable build; review artifact hashes.
4. Conduct **separate** Windows native interactive tests and browser UI tests. For unavailable devices state `NOT TESTED`, never PASS.
5. Open PR on user's connected GitHub account, include concise evidence, head SHA and working CI run links. Merge into `main` only when all required CI gates for the *current head SHA* are successful. No force-push.
6. Keep `docs/feature-coverage.md` 38 categories with exact weights (Implemented=1, Partial=0.5, Gap=0). Show numerator, denominator and rounded percentage; do not promote incomplete features based solely on source presence or green builds.
7. Publish installer and portable Windows artifacts only if successful, list NSIS signature state and SHA256. Do not assert an unsigned installer is signed.
8. Update `CHANGELOG.md`, `docs/PHASE13_REPORT.md` (separate completed / partial / gaps and browser/native QA), and `docs/NEXT_PHASE14_PROMPT.md`.

**Definition of Done:** source PR fully audited; current-head required CI all PASS; safe merged `main` SHA; Windows artifacts verified; 38-category matrix and honest native/browser reports. Any blocker means leave PR open, explain precisely, and continue from that evidence next.

# WFS Notch — Phase 15 Complete Execution Prompt

**Repo:** https://github.com/wawanwfs/WFS-Notch — continue from the ACTUAL most recent main SHA and latest green GitHub Actions. Read README, docs/PHASE14_REPORT.md, docs/feature-coverage.md, docs/security.md, docs/architecture.md, tests and relevant source before changes. Preserve Rust + Tauri 2, WFS dark/champagne-gold branding, Windows 10/11 support, accessible 16-module UI and local-first private state.

## P0 — Native Windows interactive proof (no simulated claims)

1. Use authorized real interactive Windows 10 AND Windows 11 machines. Verify NSIS installer and portable binary separately. Capture sanitized evidence, avoid user data; do not equate GitHub hosted builds or browser screenshots with physical QA. Run scripts/phase14-native-acceptance.ps1 for read-only system inventory then manually complete the checklist.
2. Verify per-app notification permissions, Focus Assist/DND, locked screen privacy and wake/sleep, time rollback, backup restore, and retry behavior. Durable at-most-once OS enqueue is not a visible toast receipt; never claim exactly-once display.
3. Verify 1–3 monitor placement, negative coordinates, full-screen versus maximized/borderless, resolution/DPI 100/125/150% changes, monitor disconnection and RDP reconnect. No focus stealing, input hooks or game anticheat interaction.
4. Verify GSMTC with Spotify, browsers and Windows Media Player, artwork, timeline progress, unsupported controls, session connect/disconnect and stable selection. AppUserModelId may not uniquely identify player instances.
5. Test keyboard navigation, reduced motion, hover interactions, color contrast and original WFS-owned WebView2 screenshots. Use golden pixel comparisons only when approved original WFS-owned reference images and stable fonts/rendering conditions exist.

## P1 — Calendar fidelity and privacy

6. Add true all-day event DATE/DTEND semantics, full calendar recurrence metadata and idempotent editing/canceling of a whole recurring series. Current Phase 14 importer flattens only bounded DAILY/WEEKLY RRULE with COUNT/UNTIL; EXDATE, RDATE, RECURRENCE-ID, BYDAY and more complex recurrence are rejected, not silently ignored. Preserve original UTC old state and migration/restore guarantees.
7. Expand timezone import/export semantics: optional VTIMEZONE aliases only when safely resolved; explicit IANA floating-time policy, DST nonexistent/ambiguous choices only with user consent, and dependency timezone-database update tests. Do not silently adjust an unknown TZID.
8. Add native signed import/export file picker only with explicit user consent; if adding OS filesystem capabilities, review Tauri IPC boundaries and strict content size/path rules. No background cloud synchronization until privacy model is agreed.
9. Continue media player and UI improvements, privacy-preserving opt-in clipboard or device controls only within supported official Windows APIs, and owner-supplied lyrics only if rights/license are clear.

## Development and release gates

- Create feat/phase15-... from latest main, real production Rust/JS changes, unit tests, migrations and browser regression. No copy of proprietary Poni source/assets or unlicensed lyrics. No public tunnel, remote shell or force-push.
- Run Windows Rust tests --all-targets --verbose, Clippy -D warnings, rustfmt --check, JS syntax, full Playwright, 64 screenshot matrix, unsigned NSIS and portable build. Keep browser and native tests separate and mark unavailable Windows interactive QA NOT TESTED.
- Keep 38-category parity exact: (Implemented + 0.5 × Partial) / 38 × 100. Starting Phase 14 3/16/19 = 28.9%; change counts only on independently defensible completed scope and tests.
- Open PR, inspect every required gate on precise PR head; merge only if ALL pass, then verify main CI and owner identity. Supply actual main SHA, PR, job and artifact URLs, installer signature state and SHA256, docs/PHASE15_REPORT.md and docs/NEXT_PHASE16_PROMPT.md.
- If a blocker persists, leave PR unmerged and report the failed gate accurately instead of claiming a release.

**Goal:** measurable real feature/QA progress with safe preservation of the functioning WFS Notch codebase.

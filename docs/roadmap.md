# Development roadmap

Current baseline: **v0.11.0 development**. Priorities are product behavior and testable Windows integration rather than documentation volume.

## Next milestones

1. Native Windows 10/11 device smoke test: 1–3 monitors, 100/125/150% DPI, fullscreen and hot-unplug/reconnect.
2. Notifications: native Rust-owned reminder scheduling and a private UI status panel now exist. Next verify Windows toast permission prompts, send failures, restart-deduplication ledger, lock-screen privacy and WFS-only pause mode. Third-party notification interception is not implemented.
3. Media: reliably select GSMTC sessions, confirm artwork decoding, media progress and playback controls in Spotify/browser/Windows Media Player.
4. UI accessibility and polished notch animations, no focus stealing in fullscreen, keyboard-first workflows.
5. Calendar recurrence/TZID, file shelf, clipboard history with explicit consent, and native device controls.

## Progress scoring

See `docs/feature-coverage.md` for the 38 feature groups. Each category is 1 (implemented in source), 0.5 (partial), or 0 (gap). The percentage is a project planning estimate, **not** verified visual or behavioral parity. Native interactive tests and reference observation are separate gates.

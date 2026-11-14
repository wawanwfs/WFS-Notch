# WFS Notch parity continuation after v0.42.0

Continue from verified `main`, not an old handoff branch. Compare against Poni 1.5.0 with original WFS assets. Do not report native comparison from browser or CI evidence.

## What shipped

- [PR #47](https://github.com/wawanwfs/WFS-Notch/pull/47) adds local task reminder snooze (5/15/60 minutes), up to 200 completion records, and backup-restore protection for snoozed and completed occurrences. A snooze does not change the recurrence anchor or turn on notifications.
- [PR #48](https://github.com/wawanwfs/WFS-Notch/pull/48) reviews every saved UTC/all-day series move, exact dates and times, reminder offsets, and same-kind overlaps. Changed inputs invalidate stale reviews. TZID/floating reflow and unsupported ICS ranges still reject.
- The fixed 38-category feature coverage score stays **28.9%**. No verified UI similarity score. Interactive Windows 10/11 remains **NOT TESTED**.

## Next implementation batches

1. Add an undoable task completion workflow with explicit occurrence identity and bounded local history retention controls. Verify redo/restore interactions, double-submit guards and private notification ledger behavior.
2. Design wall-time series moves separately from exact UTC shifts. Add explicit source zone/DST policy, and never regenerate cancelled occurrences. Preview skipped dates and remapped instants before any write. Do not loosen unsupported ICS range rejection.
3. Execute the [Windows parity matrix](WINDOWS_PARITY_ACCEPTANCE.md) on authorized Windows 10/11 hardware. Prioritize physical monitors, fullscreen and RDP, selected GSMTC session, Narrator, toast opt-in and real installer upgrade. Keep native results separate from browser screenshots.
4. Audit all 16 modules for loading, empty and failure states, contrast, keyboard access and layout across DPI. Keep original WFS visual identity and document deliberate differences from Poni 1.5.0.
5. Continue safe Shelf and consented read-only device integrations. Require bounded, local-first operations, accurate permission/error states and no new remote exposure.

Validate exact PR and post-merge CI, Rust/Clippy/format, browser evidence, installer/portable outputs and SHA-256. Keep [tracker #5](https://github.com/wawanwfs/WFS-Notch/issues/5) open until native and comparative evidence exists. Never force-push.

## Verified v0.42.0 baseline

`main` commit `cfc04afc734b7efb28a2c6e4a9ff7cb156c9d69b`, 110 Rust tests and all four CI jobs passed. [CI](https://github.com/wawanwfs/WFS-Notch/actions/runs/43613493353) · [NSIS](https://github.com/wawanwfs/WFS-Notch/actions/runs/43613493353/artifacts/14121491758) · [Portable](https://github.com/wawanwfs/WFS-Notch/actions/runs/43613493353/artifacts/14121521685) · [SHA-256](https://github.com/wawanwfs/WFS-Notch/actions/runs/43613493353/artifacts/14121491767) · [Browser evidence](https://github.com/wawanwfs/WFS-Notch/actions/runs/43613493353/artifacts/14121615449).

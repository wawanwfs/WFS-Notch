# WFS Notch next parity batch after v0.40.0

Continue from the current `main`. Compare against Poni 1.5.0, with original WFS assets and behavior documented separately. Do not infer native coverage from browser screenshots or Windows packaging.

- v0.39.0 preserves current cancellation decisions across explicit backup restore, rejects stale import reviews, and prevents old backups from undoing an explicit history reset. [PR #44](https://github.com/wawanwfs/WFS-Notch/pull/44) · [exact main CI](https://github.com/wawanwfs/WFS-Notch/actions/runs/43607496981).
- v0.40.0 adds optional IANA wall-clock repeats without changing elapsed schedules. An initial missing time is rejected, future gaps are skipped, and overlaps use the first instant. [PR #45](https://github.com/wawanwfs/WFS-Notch/pull/45) · [`1bca994`](https://github.com/wawanwfs/WFS-Notch/commit/1bca994ab73ee537135f01c0e4568e855a482a61) · [exact main CI](https://github.com/wawanwfs/WFS-Notch/actions/runs/43607862324).
- The fixed 38-category coverage score remains **28.9%**. Interactive side-by-side Windows 10/11 remains **NOT TESTED**. Use the [Windows acceptance matrix](WINDOWS_PARITY_ACCEPTANCE.md).

## Next implementation batches

1. **Task snooze and history.** Add an explicit, bounded snooze and a local completion history without shifting the recurrence anchor or replaying notifications on backup restore. Test elapsed and calendar schedules, old snapshots, simultaneous edits and DST.
2. **Calendar schedule preview.** Show exact affected local dates, skipped times, reminder impact and conflicting events before any series edit. Keep unsupported ICS ranges fail-closed. Add stale-preview, metadata round-trip and restore regression coverage.
3. **Native Windows comparison.** With authorized Windows 10 and 11 access, verify display scaling, physical monitor hotplug, fullscreen/RDP, GSMTC sessions, WebView2 keyboard and screen reader, toast consent and installed/portable upgrade. Without access, record NOT TESTED and continue independent work.
4. **UI and remaining parity features.** Audit all 16 modules with empty/loading/error states and keyboard navigation. Prioritize features backed by observable reference behavior, including safe local Shelf and read-only device controls. Preserve original WFS design.

Keep exact PR and post-merge CI, artifact SHA-256, all browser and Rust gates, and the [master tracker](https://github.com/wawanwfs/WFS-Notch/issues/5) current. Never force-push or change the fixed parity score without full category evidence.

## Verified v0.40.0 artifacts

Exact main CI passed all four jobs and 108 Rust tests. [NSIS](https://github.com/wawanwfs/WFS-Notch/actions/runs/43607862324/artifacts/14118927703) · [Portable](https://github.com/wawanwfs/WFS-Notch/actions/runs/43607862324/artifacts/14119630836) · [SHA-256](https://github.com/wawanwfs/WFS-Notch/actions/runs/43607862324/artifacts/14119067508) · [Browser screenshots](https://github.com/wawanwfs/WFS-Notch/actions/runs/43607862324/artifacts/14118194775).

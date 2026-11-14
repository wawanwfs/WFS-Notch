# WFS Notch — next parity batch after v0.38.0

Continue from the current `main` in `wawanwfs/WFS-Notch`. Keep the reference fixed to **Poni 1.5.0** and use original WFS branding and assets. Implement and test successive independent changes without waiting for another short prompt. Never force-push or claim native behavior from CI.

## Source baseline and evidence

| Scope | Source commit | Exact main CI |
| --- | --- | --- |
| v0.36.0 — module finder, Weather, Pomodoro, Countdown | `3dd4ed347a76cfc14de5a6e58f6a26b7e5806fd6` | [43474853458](https://github.com/wawanwfs/WFS-Notch/actions/runs/43474853458) |
| v0.37.0 — reviewed scoped ICS cancellations | `13e91e30f95542d0c43d3a62a9c533d944077b2f` | [43602884272](https://github.com/wawanwfs/WFS-Notch/actions/runs/43602884272) |
| v0.38.0 — repeating local tasks | `7a26de1085bb07ece8ee947ce0c266fc7036c150` | [43603228341](https://github.com/wawanwfs/WFS-Notch/actions/runs/43603228341) |

Both v0.37.0 and v0.38.0 exact-main workflows passed all four jobs. v0.38.0 passed **105 Rust tests**, strict Clippy and formatting, and the full browser suite with the 64-case screenshot matrix. Windows packaging is unsigned. [Installer](https://github.com/wawanwfs/WFS-Notch/actions/runs/43603228341/artifacts/14117450911) · [Portable](https://github.com/wawanwfs/WFS-Notch/actions/runs/43603228341/artifacts/14117535613) · [SHA256](https://github.com/wawanwfs/WFS-Notch/actions/runs/43603228341/artifacts/14117535603) · [Browser evidence](https://github.com/wawanwfs/WFS-Notch/actions/runs/43603228341/artifacts/14116903852).

- [PR #41](https://github.com/wawanwfs/WFS-Notch/pull/41) adds a read-only ICS preview, stale-preview check, bounded exact occurrence cancellation, anti-resurrection markers and faithful export. Whole-series/range cancellation, iTIP scheduling messages and unsupported recurrence shapes still reject. Conflicting local edits are never removed by the import.
- [PR #42](https://github.com/wawanwfs/WFS-Notch/pull/42) adds completion-driven 24-hour and seven-day repeats with one stable task ID and due-date replay protection. These are elapsed durations, not calendar-time recurrences across DST. Existing notification consent is unchanged.
- The fixed 38-category REA feature score stays **28.9%**. There is no verified UI similarity percentage. Interactive Windows 10/11 side-by-side verification remains **NOT TESTED**.

## Next independent priorities

1. **Calendar backup restore consistency.** Audit restoring a backup older than a scoped ICS cancellation. Preserve safe marker semantics with the restored source metadata, avoid unexpected resurrection, and cover explicit user reset and stale previews. Then improve exact preview for series schedule changes and DST without relaxing rejection of unsupported shapes.
2. **Tasks and reminders.** Add opt-in calendar-time recurrence with explicit timezone and DST overlap/gap policy, followed by snooze, bounded completion history and a restore-safe notification journal. Keep the existing elapsed schedule for previously configured tasks unless the user explicitly changes it.
3. **Windows display and GSMTC.** Validate physical 1–3 monitors, mixed DPI, fullscreen and RDP on authorized Windows machines. Verify real player capabilities and continue to refuse ambiguous media-session control.
4. **UI coverage.** Audit each module's functional states, keyboard and screen-reader paths, contrast, layout and DPI. Compare on WebView2 with Poni 1.5.0 while retaining WFS branding. Keep browser screenshots as regression evidence, not native UI proof.
5. **Installer and product scope.** Verify upgrade and recovery with real Windows 10/11, then continue the missing feature groups in the master matrix, including consented device integrations, Shelf and safe local data tools.

For each change, compare exact PR and post-merge source commits and require green Rust, Clippy, format, browser and package jobs. Record screenshots and SHA256 artifacts. Check [the master plan](NEXT_PHASE28_MASTER_PROMPT.md) and [parity tracker](https://github.com/wawanwfs/WFS-Notch/issues/5) before starting.

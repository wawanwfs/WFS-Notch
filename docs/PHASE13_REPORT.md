# Phase 13 — Deterministic Native Reliability & Backup-Safe Reminders

**Development target:** WFS Notch v0.13.0, Rust + Tauri 2, original WFS dark/champagne-gold identity.  
**Start:** `main` `b62e9c4e7c9d72369e0a108c8de8de08fef2ad4c`, Phase 12 CI https://github.com/wawanwfs/WFS-Notch/actions/runs/42482654316 (all PASS).  
**Working branch:** `feat/phase13-reminder-recovery-qa`.

## Changes implemented in source

1. **Real transactional delivery seam.** `notifications::deliver_with` takes an injected UTC-milliseconds timestamp and sender function but calls the same `Store::transact` and durable reservation logic as the Windows worker. Tests cover real disk commits, restart, repeated polling, changed reminders, paused/disabled settings and platform send errors. `due_event_ids` makes scheduling pure and deterministic.
2. **Clock rollback protection.** Delivery reservations remain eligible for cleanup after 30 days from the event's original timestamp instead of five minutes after its end. This preserves recent history across system-time rollback and normal app restarts. Finite retention means it cannot prevent replay after extreme rollback or corrupted/lost state.
3. **Backup restore idempotency.** When user data is restored from an earlier backup, delivery entries from the current state are merged into the restored state's ledger. Existing reservation expiry values always win over older entries. Local JSON contents, notes, events and settings still restore normally.
4. **Fullscreen false-positive regression.** Foreground windows spanning the width or height of two monitors no longer automatically count as fullscreen on the selected monitor. Tests include signed virtual-screen origins, maximized work-area exclusion and 100/125/150% DPI geometry.
5. **Documentation/security corrections.** Updated privacy text to reflect durable ledger, source version and explicit at-most-once enqueue vs visible notification semantics.

## Current limitations / partial implementations

- **Native Windows 10/11 interactive QA: NOT TESTED.** A GitHub Windows runner compiles and executes Rust tests and builds NSIS, but does not prove toast visibility, Focus Assist behavior, monitor hotplug/fullscreen focus handling, RDP reconnect, Spotify/Chrome/Windows Media Player session controls, or WebView2 UI functionality on physical devices.
- **Exactly-once visible notifications: NOT SUPPORTED.** The design reserves before an OS call; a crash between durable reservation and OS enqueue can miss one reminder. A reply from the OS API does not prove the user saw it.
- **Clock rollback protection: BOUNDED.** A 30-day journal is a conservative size/performance tradeoff; manual clock rollback beyond retention or restoring a corrupted state may still replay an event.
- **Calendar/TZID/DST:** calendar event scheduling operates on absolute UTC epoch milliseconds. Full TZID/floating-date ICS parsing, recurrence and automatic calendar sync are still missing. A test harness cannot simulate missing timezone features.
- **GSMTC multi-session:** the app uses SourceAppUserModelId which is not a unique per-instance identity. No claimed proof on actual player devices. Audio/media ownership and lyrics copyright rules remain unchanged.
- **Visual golden comparisons:** 64 Chromium screenshots exist for 16 WFS-owned modules across simulated DPI. Approved WFS-owned visual baseline images were not supplied; no pixel-accurate golden comparison is claimed or created. Real Windows WebView2 screenshots remain untested.
- **Reference tools:** REA/Craft Anything/LeanREA/Ghidra were not used for proprietary source or assets. No lawful vendor binaries were made available for this phase.

## QA gates and evidence

**Automated:** GitHub Actions `.github/workflows/ci.yml` — Windows Rust tests, Clippy with `-D warnings`, rustfmt, browser regressions, screenshot matrix, unsigned NSIS and portable Windows executable. Only successful runs for the **current branch head** count. PR and final workflow links are tracked in GitHub.

**Separate manual native acceptance required:**

| Environment/scenario | Required observable proof | Status |
| --- | --- | --- |
| Windows 10 NSIS and Windows 11 portable | Toast on/off, WFS-only pause, Focus Assist/DND, lock screen and privacy settings | NOT TESTED |
| Sleep/resume and time changes | One reminder attempt across repeated launch/suspension, no duplicates after backup restore | NOT TESTED |
| 1–3 physical monitors | Fullscreen/borderless/maximized, primary/secondary, negative origins, hotplug, resolution and 100/125/150% DPI | NOT TESTED |
| Windows RDP / reconnect | Placement and focus behavior across virtual display topology changes | NOT TESTED |
| Multiple GSMTC players | Current session, source selection, session disconnect, capability flags, progress and artwork | NOT TESTED |
| WebView2 accessibility | Actual Tab/Arrow focus navigation, reduced-motion, hover and expanded/collapsed transitions | NOT TESTED |

## 38-category REA-derived weighted feature accounting

`3 Implemented × 1 + 16 Partial × 0.5 + 19 Gap × 0 = 11 points`.  
`11 ÷ 38 × 100 = 28.947...% = 28.9%` (one decimal).

No category has been promoted because native device integration and feature breadth remain incomplete. Static source and green CI do not equal 100% parity.

## Release and merging

Never force-push. Open a feature PR, inspect all GitHub Actions for its latest HEAD, merge only when Rust, Clippy, rustfmt, browser evidence and unsigned NSIS+portable artifact gates are PASS. Installer is **unsigned**. Source merge does not itself constitute a signed or user-validated Windows release.

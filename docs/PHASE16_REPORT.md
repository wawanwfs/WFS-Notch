# WFS Notch Phase 16 — Timed Calendar End & Durable ICS Cancellations

**Repository:** https://github.com/wawanwfs/WFS-Notch  
**Starting main:** d24da8b8ad95d7a7c69a462ab2c97fd799d58ea8 (v0.15.0, CI run 42494544548 PASS).  
**Working branch:** feat/phase16-timed-calendar-durable-cancel; source version 0.16.0.  
**CI evidence:** Read the linked current-head PR and latest successful main GitHub Actions run. Do not conflate earlier passing runs with newer SHA.

## Actual functional changes

1. **Persisted UTC exclusive end for timed events.** Backward-compatible, optional ends_at_ms field in CalendarEvent. Existing timed or all-day JSON backups, notification IDs, titles and recurrence identifiers deserialize without migration. Start must precede end, maximum span 366 elapsed days and last supported UTC end 2100-01-01. Invalid persisted end times are excluded during sanitization rather than silently accepted.
2. **Safe RFC 5545 timed DTEND and DURATION.** ICS reader resolves UTC, IANA TZID, and explicitly chosen floating IANA timezone for DTSTART/DTEND. Rejects ambiguous/nonexistent DST wall times, invalid or zero/negative duration, both DTEND+DURATION, mixed DATE and datetime, unsupported property parameters and partial event batches. Supports bounded positive RFC 5545 duration subset (PnW, PnD, PnDTnHnMnS, PTnHnMnS, no fractional/negative components). Writer exports explicit UTC DTSTART/DTEND for timed events and keeps civil DATE/DTEND for all-day events.
3. **Recurring duration scope.** A source event with a valid elapsed duration expands to independent occurrences with the SAME elapsed UTC duration after each recurrence start has been resolved in its timezone. This is not original RRULE/VTIMEZONE series serialization or a promise of wall-clock duration semantics for every RFC5545 variant.
4. **Durable local deletion history.** A deleted imported event records its safe ICS occurrence ID; deleting a series records its safe source series ID. Both are kept in a bounded title-free local HashSet calendar_tombstones, serialized in local JSON/backups, and honored on subsequent explicit ICS imports. This prevents unintended resurrection without a privacy-invasive cloud sync or title storage. A visible, explicitly confirmed reset clears cancellation markers but never auto-imports removed content.
5. **Accessible calendar UI.** Optional exclusive timed end, editable private description, timed multi-day calendar badges using local midnight boundaries (including DST days), consistent browser preview storage, and a scoped reset control. All-day DATE logic, existing reminder scheduling and the prior 16 modules remain.
6. **QA scripts and tests.** Rust unit tests cover UTC/IANA DST end timestamps, duration parser rejection, recurring elapsed duration, legacy JSON, scoped tombstones and reimport; Playwright covers timed edits, description persistence, day badges and manual reset. scripts/phase16-native-acceptance.ps1 is read-only, prints hashes and sanitized monitor inventory, and records user-reported manual PASS/FAIL or NOT TESTED statuses. It does NOT actually test hardware behavior.

## Explicitly not completed or tested

- **Windows 10/11 interactive hardware: NOT TESTED.** GitHub Windows CI compiles/packages binaries but does not prove native toast delivery or user-visible receipts, Focus Assist/DND, lock-screen events, fullscreen, monitor hotplug, 100/125/150% physical DPI, RDP reconnect, GSMTC real media apps, NSIS install/uninstall, or accessibility of actual WebView2 on a physical device. User-reported manual QA is tracked separately from automation.
- Golden-image pixel diff: **NOT TESTED**, no approved WFS-owned reference screenshot baseline.
- Full source-series schedule editing, stable imported recurrence-rule metadata, true RECURRENCE-ID override semantics, complex BYDAY/BYMONTH/monthly/yearly RRULE, arbitrary VTIMEZONE aliases, original RRULE preservation on ICS export, Google/Outlook cloud sync and native file picker: **NOT IMPLEMENTED**.
- Manual backup restoration may deliberately change events and cancellation markers to the backed-up state; deliberate import history reset may also restore removed events on the next explicit import.
- Exactly-once visible Windows notifications: **NOT SUPPORTED**. Existing journal is best-effort at-most-once enqueue attempt, not an OS visibility receipt.
- No proprietary third-party source/assets/lyrics, no privileged OS hooks, no public tunnel, no signed release.

## Local Windows 10/11 acceptance workflow

On an authorized Windows machine (no administrator permission required to run inventory):
1. Download unsigned Windows NSIS ZIP, unsigned portable ZIP and SHA256 file from the final successful main CI of v0.16.0. Extract locally and verify SHA256.
2. In a PowerShell console inside the repository folder, run:
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\phase16-native-acceptance.ps1 -InstallerPath "C:\path\WFS Notch_0.16.0_x64-setup.exe" -PortablePath "C:\path\wfs-notch.exe" -Interactive
3. Actually test Windows 10 and 11 systems separately, including start/end dates, DST, installer upgrades, reminders, fullscreen, monitor hotplug, GSMTC and WebView2. Only then manually mark observed results. The script does not install or run anything and stores a report locally under LOCALAPPDATA\WFS-Notch\NativeQA.

## Source/CI acceptance and weighted parity

The exact current PR head, then merged main, must pass: Windows Rust cargo test --all-targets --verbose, cargo clippy -D warnings, cargo fmt --check; Node/static audits; full Playwright suite including Phase15 and new Phase16 plus 64 browser screenshots; unsigned NSIS and portable builds; and owner push identity on main. Final artifact URLs and SHA256 must be extracted from final merged main CI.

REA-derived fixed 38-category coverage remains **3 Implemented / 16 Partial / 19 Gap**, weighted = (3 + 16 × 0.5) / 38 × 100 = **28.9%**. Calendar improvements remain within an existing Partial category. Native interactive QA is separate.

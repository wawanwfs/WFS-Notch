# WFS Notch

**A private, local-first desktop companion for Windows 10/11 — built with Rust and Tauri 2.**

![WFS Notch](assets/branding/wfs-notch-wordmark.png)

WFS Notch brings an expandable top-edge panel with a calendar, tasks, notes, focus timers, native Windows media controls, device information and optional private desktop reminders. It does not require an account.

## Features

- Lightweight notch with opt-in Win32 fullscreen-aware topmost handling, stable unique display-name selection across screen rearrangement, safe legacy coordinate fallback, DPI/topology re-positioning, tray and autostart.
- Two retained local state recovery generations (`state.json.bak` and `state.json.bak.prev`) with startup fallback when primary/latest backup are unreadable; no destructive installer migration.
- Local accessibility preferences: compact or comfortable density, optional minimal motion, high contrast and safe reset; stored in the same private native JSON with backwards compatibility.
- Notes backup restore advances revisions for shared IDs so stale editor windows cannot overwrite restored content.
- Local Notes JSON preview/import/export (strict 1 MiB / 250-note snapshots, dedup by ID and atomic append-only import), explicit user copy/paste with no network.
- Conflict-safe Notes: versioned optimistic saves, local crash-recoverable drafts, explicit reload or save-as-new resolution without overwriting newer edits. No cloud sync.
- Private local Notes tags (up to 8), tag-aware search and an accessible filter; older notes load without destructive migration.
- Task editor with local priority (low/normal/high), optional due date, safe update preserving task ID/completion, and non-destructive legacy migration (separately opt-in local task due Windows reminders with at-most-once enqueue ledger); note tools; calendar with safe RFC5545 single-occurrence RECURRENCE-ID overrides, original UID/stable instance identity and fail-closed range/DST handling; UTC/IANA, true all-day DATE and timed exclusive DTEND/DURATION, multi-day view, bounded DAILY/WEEKLY/MONTHLY/YEARLY recurrence with BYDAY/BYMONTH/BYMONTHDAY/WKST, EXDATE/RDATE exceptions, locally persisted versioned source rules, durable cancellation history, scoped group rename/delete, and a guarded one-time whole-day move of untouched UTC/all-day recurring instances; wellness and focus timers.
- Windows GSMTC media sessions: selected-player metadata, capability-checked playback controls, artwork when supplied by the player and bounded progress. Shared application IDs (e.g. multiple browser tabs) are now explicitly flagged as ambiguous and cannot receive per-session playback commands; use the unique app session or system media keys instead.
- Native opt-in calendar and separately consented task due notifications with a Rust scheduler, durable at-most-once enqueue ledger, persistent bounded retry journal (up to three sender attempts), and Windows toast setting diagnostic that run independently of the hidden notch panel. WFS Notch never reads other applications' notifications. Event titles stay private unless enabled, sender error logs omit titles and imported UIDs, and WFS-only Pause Reminders is available. A successful OS enqueue is not proof a toast was displayed, and ambiguous Windows failures can still lead to a bounded duplicate attempt.
- Read-only local disk/network/CPU/RAM inventory, theme colors and accessible keyboard navigation.
- Local JSON with backup and restore.

iCalendar DATE/DTEND is stored as true timezone-neutral civil dates with an exclusive end; all-day events do not fire unintended midnight reminders. Unsupported RECURRENCE-ID overrides, non-representable RFC5545 recurrence parts (such as BYSETPOS) and DST-ambiguous/nonexistent IANA local timestamps are rejected rather than guessed. Timed DTEND and positive DURATION (up to 366 days) use a strict elapsed UTC duration for materialized occurrences, with no guessed timezone. Supported RRULE events are materialized as independently editable occurrences: rename/delete the imported group, or edit one occurrence. Deleted imported occurrences/series remain canceled on subsequent explicit imports until the user deliberately clears their local cancellation markers. Full recurrence source schedule editing, including IANA/floating DST reflow, arbitrary RRULE revisions, RECURRENCE-ID overrides and automatic subscriptions, remains out of scope. A one-time local whole-day series shift is available only with preview and explicit confirmation for unchanged UTC/all-day source-backed series, retaining IDs, cancellation tombstones and reminder history; all moved occurrences are marked edited and exported individually. Export now preserves one validated RRULE master for an unchanged imported series, but safely exports independent events if local occurrences have been edited, deleted, canceled or lack trustworthy source metadata. This fallback deliberately does not preserve recurrence semantics, and the user should review the resulting ICS before sharing. The original recurrence rule is now stored locally for new imported series; existing flattened legacy series cannot be reverse-inferred reliably. The local-only manual Windows acceptance checklist is `scripts/phase14-native-acceptance.ps1`.

## Project layout

```text
src/                       HTML entry, app state and asset links
  features/                Calendar, media, wellness, system and display UI
  styles/                  Base layout and feature styles
  assets/                  Frontend SVG
src-tauri/                 Rust 2021 / Tauri 2 native Windows application
  src/                     Native modules and commands
  capabilities/            IPC permissions
  icons/                   Windows app icons
tests/                     Browser regressions and static source checks
assets/branding/           Original WFS Notch identity
docs/                      Architecture, security, roadmap, coverage
scripts/                   Build helper
.github/workflows/ci.yml   Windows build, Rust tests and browser CI
```

## Develop and build on Windows

Install Rust (MSVC toolchain), C++ Build Tools and Windows SDK, WebView2 Runtime, and Tauri CLI 2. No `winget` dependency.

```powershell
git clone https://github.com/wawanwfs/WFS-Notch.git
cd WFS-Notch
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
.\scripts\build-windows.ps1
```

For a static browser preview, open `src/index.html`. Browser mode is not an emulator and does **not** fabricate native Windows media, monitors, notifications or device data.

## Security and product maturity

Native notifications are off by default, task due alerts require a second opt-in and unfinished tasks with due times, and private event/task titles are hidden until the user opts in. The notification pause option affects only WFS Notch. All user data remains on the device except features where the user explicitly requests third-party weather or opens an external link. No public tunnel or remote shell is deployed.

**Phase 13 development target: v0.13.0.** Windows CI builds and regression tests verify code quality and package creation, not real-device behavior or complete equivalence to any other product. This is a development release, unsigned and not independently certified on physical Windows machines.

Read [Phase 13 Report](docs/PHASE15_REPORT.md), [Next Phase 14 Prompt](docs/NEXT_PHASE16_PROMPT.md), [Architecture](docs/architecture.md), [Security](docs/security.md), [Feature coverage](docs/feature-coverage.md), [Phase 12 Report](docs/PHASE12_REPORT.md), [Phase 13 Prompt](docs/NEXT_PHASE13_PROMPT.md), and [Roadmap](docs/roadmap.md).

## License

No open-source license has been granted yet. Rights are reserved by the repository owner until a license is published.


## Continued development (Phase 28+)

The [Phase 28 autonomous master execution prompt](docs/NEXT_PHASE28_MASTER_PROMPT.md) records the verified v0.27.0 baseline, artifact links, unfinished A–J workstreams and independent PR/CI acceptance gates. The [full historical accelerated backlog](docs/NEXT_ACCELERATED_BATCH_PROMPT.md) remains available. Browser screenshots and GitHub Actions packaging are **not** evidence of native Win10/11 interactive behavior; local verification is documented in `scripts/phase16-native-acceptance.ps1`.

# Feature coverage — WFS Notch

**Method.** Independent static examination of two user-supplied Poni 1.5.0 executables plus the published feature guide/release notes and WFS Notch source. No original vendor source or copyrighted assets were copied. Reference executables were **not run** in this environment. This report distinguishes confirmed functionality from unverified work; it does not claim 100% parity.

## Reference scope

- Published guide: https://poni.web.id/panduan/
- 1.5.0 release notes: https://poni.web.id/catatan-rilis/

Legend: **Implemented** means source exists; CI/native status is separate. **Partial** lacks reference capabilities. **Gap** is not yet implemented.

| Reference feature | WFS Notch 0.14.0 source | Remaining difference / evidence required |
| --- | --- | --- |
| Compact/expanded interactive notch | Partial | DPI-aware manual placement plus opt-in Win32 foreground fullscreen guard (excluding spanning windows) / display hotplug polling; real device/hover/pass-through QA still pending |
| Closed notch information | Partial | Running timer/focus countdown now shown; configurable sides, events, task, battery, weather missing |
| Appearance / glass / animations | Partial | Accent, reduced-motion-friendly compact/expanded transitions and narrow-viewport scrolling; reference glass/animation library/layout editor missing |
| Music & transport controls | Partial | GSMTC selected session metadata, artwork, timeline normalization and control capability checks; lyrics and native Windows player regression missing |
| Synced lyrics | Gap | LRCLIB matching, offsets, romanization |
| Desktop calendar | Partial | Month grid, UTC/IANA TZID/explicit floating IANA timezone ICS text import, bounded DAILY/WEEKLY expansion, UTC ICS export and local reminders; all-day date semantics, full RRULE/EXDATE, series editing, file picker, subscriptions and Google/Outlook sync missing |
| Todo | Partial | CRUD; reminders, recurring, natural language parsing, snooze missing |
| Notes | Partial | Local CRUD/autosave, full-text search and question::answer flashcards; rich editing and tagging missing |
| Focus/Pomodoro | Partial | Preset focus; automatic work-break cycles and sounds missing |
| Stopwatch | Implemented | Browser UI + localStorage; native runtime still unverified |
| Independent timer | Implemented | 1–1440 min, pause, +1/+5, persistent Rust state; native runtime to verify |
| Health | Partial | Water/break/calm tracker, daily reset; background reminders and activity telemetry missing |
| Day Progress | Partial | Time and today events/tasks; wake/sleep schedule and richer timeline missing |
| Weather | Partial | Manual city current/daily via Open-Meteo; 7-day/hourly, geolocation missing |
| Prayer Times | Gap | Kemenag/Aladhan integration and reminders |
| System Monitor | Partial | CPU/RAM, per-volume disk capacity and per-interface network byte totals; GPU/temperature and network controls missing |
| Screen Time | Gap | Per-app foreground activity not tracked |
| Wi-Fi Controls | Gap | Windows adapter & network list APIs |
| Bluetooth Controls | Gap | Windows radio/device APIs, pairing/battery |
| Dashboard quick toggles | Gap | Wi-Fi, Bluetooth, Keep Awake, dark mode, brightness, mic |
| Notifications | Partial | Rust-native background scheduler, durable per-event at-most-once reservation ledger with bounded clock rollback retention, deterministic transaction tests and backup restore replay protection, catch-up after resume, opt-in private titles, WinRT toast setting diagnostic and WFS-only pause; OS delivery acknowledgement, full system DND, native tests and third-party listener missing |
| HUD/OSD | Gap | Volume, brightness, keyboard state, airplane mode overlays |
| Dock | Partial | Quick launch/links only, not running-app/taskbar dock |
| Shelf/drop actions | Gap | File drops, ZIP, convert, safe send |
| Clipboard history | Gap | Consent-driven capture & privacy exclusions |
| Snap zones | Gap | Native window tiling/drag interactions |
| AI Chat / translate | Gap | Provider/credential management, local Ollama, tools, undo |
| AI plan usage | Gap | Claude/Codex local usage, quotas, rings |
| Claude Code interaction | Gap | Permission requests/questions/notifications |
| GitHub module | Gap | OAuth/PR review lists |
| FPS counter | Gap | Windows capture/permission path |
| Radio | Gap | Radio Browser, favorites, playback coordination |
| Bookmarks | Partial | Quick web links; categorization/filter missing |
| Buddy | Gap | Independent animated character |
| Mouse cursors | Gap | Windows cursor pack and restore |
| Native autostart/tray | Implemented | Windows runner compilation; behavioral device test pending |
| Data backup | Partial | Local JSON snapshots/restore; portable import/export missing |
| Self-update | Gap | Signed updater, portable notification |

## Phase 14 weighted score (38 categories)

Counts: **Implemented 3**, **Partial 16**, **Gap 19**. Formula: (3 × 1 + 16 × 0.5 + 19 × 0) / 38 × 100 = **28.9%** (rounded to one decimal). This remains unchanged from v0.13.0: significant progress *within* partial categories does not promote an entire category to Implemented without completing its scope. CI results and real Windows tests are tracked separately in `docs/PHASE14_REPORT.md`.

## Gate for accurate 100% claim

Every reference feature must pass: (1) independently observed behavior or reliable specification, (2) implementation and automated coverage, (3) native Windows interactive test, (4) regression, (5) privacy and performance validation. For hardware/Windows-only features, a clean CI build is **not** proof of behavioral parity.

### Next native integration milestones
Windows GlobalSystemMediaTransportControlsSessionManager + album art/progress; native winrt notifications; Bluetooth/Wi-Fi; real taskbar dock; filesystem shelf; privacy-preserving AI providers; 1.5.0 AI usage and Claude Code capabilities.

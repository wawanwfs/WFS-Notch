# REA functional parity matrix — Poni 1.5.0 versus WFS Notch 0.6.0

**Method.** Independent static examination of two user-supplied Poni 1.5.0 executables plus the published feature guide/release notes and WFS Notch source. No original vendor source or copyrighted assets were copied. Reference executables were **not run** in this environment. This report distinguishes confirmed functionality from unverified work; it does not claim 100% parity.

## Reference evidence

- Portable: SHA-256 `4c0ee8719d474eed32389352d41874ab11b1541272a3886f63f3ff5cc62388ce`; PE32+ x64; 11,895,808 bytes.
- Setup: SHA-256 `e45f95d75e47d4869321e64507d2999931868f9c3971e8681ec9277007a5fdde`; NSIS PE32; 4,973,625 bytes.
- Published guide: https://poni.web.id/panduan/
- 1.5.0 release notes: https://poni.web.id/catatan-rilis/
- WFS Notch prior baseline: 0.4.0, GitHub Actions successful before this REA stage.

Legend: **Implemented** means source exists; CI/native status is separate. **Partial** lacks reference capabilities. **Gap** is not yet implemented.

| Reference feature | WFS Notch 0.5.0 | Remaining difference / evidence required |
| --- | --- | --- |
| Compact/expanded interactive notch | Partial | Exact hover, mouse pass-through, auto-hide, multi-monitor, fullscreen handling |
| Closed notch information | Partial | Running timer/focus countdown now shown; configurable sides, events, task, battery, weather missing |
| Appearance / glass / animations | Partial | Accent + collapse; glass, frosted, 9 animations, layout editor missing |
| Music & transport controls | Partial | Global media keys only; Windows session title, album art, progress, player list missing |
| Synced lyrics | Gap | LRCLIB matching, offsets, romanization |
| Desktop calendar | Partial | Month grid, UTC ICS text import/export and local reminders; TZID/floating-date support, file picker, iCal subscriptions and Google/Outlook sync missing |
| Todo | Partial | CRUD; reminders, recurring, natural language parsing, snooze missing |
| Notes | Partial | Local CRUD/autosave, full-text search and question::answer flashcards; rich editing and tagging missing |
| Focus/Pomodoro | Partial | Preset focus; automatic work-break cycles and sounds missing |
| Stopwatch | Implemented | Browser UI + localStorage; native runtime still unverified |
| Independent timer | Implemented | 1–1440 min, pause, +1/+5, persistent Rust state; native runtime to verify |
| Health | Partial | Water/break/calm tracker, daily reset; background reminders and activity telemetry missing |
| Day Progress | Partial | Time and today events/tasks; wake/sleep schedule and richer timeline missing |
| Weather | Partial | Manual city current/daily via Open-Meteo; 7-day/hourly, geolocation missing |
| Prayer Times | Gap | Kemenag/Aladhan integration and reminders |
| System Monitor | Partial | CPU and RAM only; disk/GPU/temp/network missing |
| Screen Time | Gap | Per-app foreground activity not tracked |
| Wi-Fi Controls | Gap | Windows adapter & network list APIs |
| Bluetooth Controls | Gap | Windows radio/device APIs, pairing/battery |
| Dashboard quick toggles | Gap | Wi-Fi, Bluetooth, Keep Awake, dark mode, brightness, mic |
| Notifications | Gap | Windows listener, history, privacy, DND |
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

## Gate for accurate 100% claim

Every reference feature must pass: (1) independently observed behavior or reliable specification, (2) implementation and automated coverage, (3) native Windows interactive test, (4) regression, (5) privacy and performance validation. For hardware/Windows-only features, a clean CI build is **not** proof of behavioral parity.

### Immediate next native integration tranche
Windows GlobalSystemMediaTransportControlsSessionManager + album art/progress; native winrt notifications; Bluetooth/Wi-Fi; real taskbar dock; filesystem shelf; privacy-preserving AI providers; 1.5.0 AI usage and Claude Code capabilities.

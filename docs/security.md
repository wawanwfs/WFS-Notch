# Security and privacy

- **Data:** tasks, notes, calendar events, settings and backups are stored locally. No account or central telemetry.
- **Windows media:** read from supported GSMTC sessions; data is shown locally. Album artwork is accepted only when the player provides a limited-size PNG/JPEG/GIF thumbnail.
- **Notifications:** only WFS-created calendar reminders. Disabled by default, redacted titles by default, and optional WFS-only pause mode. The native 20-second scheduler runs only while the WFS Notch process is running, requires opt-in on every dispatch, never scans third-party notifications, and does not persist private titles in logs. Reminder attempts are reserved in a title-free ledger in the app's local state before enqueue and retained for a bounded period across restarts. A platform API rejection removes its reservation for retry. This provides best-effort at-most-once enqueue attempts, not exactly-once visible delivery; a crash after reservation but before the OS accepts a toast can result in a missed notification. Backup restore preserves the current delivery journal. WFS does **not** intercept notifications from other apps or alter Windows global Do Not Disturb.
- **Calendar:** ICS import is limited to 256 KiB and 250 expanded events; source DTSTART timestamps with IANA TZID are converted locally to absolute UTC. Floating times require explicit user-supplied IANA timezone. DST gaps/overlaps, all-day date semantics, unsupported recurrence exceptions, and unbounded recurrence are rejected atomically rather than guessed. Repeated imports deduplicate stable UIDs. Basic recurring event import is flattened into standalone UTC events; source recurring-series semantics are not preserved for editing/export. No remote calendar fetch or system calendar access.
- **Display:** enumeration and local positioning only, with optional read-only foreground fullscreen rectangle checks; no admin privileges, input hooks, process-title collection, or OS monitor configuration changes.
- **IPC:** Tauri capabilities limit frontend access. User-provided links are constrained to HTTP(S). Do not store secrets or remote management credentials in this repository.
- **CI:** read-only workflow token, no public tunnel, no shell exposed from a runner, no original Poni installers, no raw Ghidra pseudocode or reference executables.

## Known limits

Unverified Windows hardware behavior, unsigned installer, no independent penetration test, and limited privacy controls on external services requested by the user. Some focus/calendar reminders are in-app only unless the native app is running.

## Windows installer reproducibility

CI fetches the official NSIS 3.13 bundle and Tauri NSIS utility DLL only from Tauri's own GitHub Releases/API, checks their upstream-pinned SHA-1 digests, and uses the per-user Tauri cache. This is an integrity match to Tauri's bundled pins, **not** a replacement for code signing or stronger artifact attestations. Downloads are retried to mitigate GitHub CDN 504 errors; unsigned installers remain explicitly labeled.

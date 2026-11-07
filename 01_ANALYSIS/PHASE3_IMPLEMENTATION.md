# Phase 3 — implementation decisions and known limits

## Engineering changes

1. `AppData.events` is a backward-compatible, defaulted `Vec<CalendarEvent>`; invalid entries are removed when a state file is opened or a backup is restored. Create/edit commands share the same validation.
2. Event timestamps use milliseconds since the Unix epoch (UTC); `datetime-local` in the UI is converted to UTC milliseconds on submission. Rendering uses OS-local timezone.
3. Reminder offset is optional, or one of 0, 5, 15, 30, or 60 minutes. Display is opportunistic when the webview is visible, at most once per event per app session; events more than 10 minutes overdue do not display reminders.
4. `System` is created once and held in `Mutex<System>` instead of constructing a new sampler each time. This supports delta-based CPU sampling. Polling occurs only while the System tab is active, every 4 seconds.
5. Manual backups store the full JSON AppData inside `app_data_dir()/backups`. The frontend never sends arbitrary file paths to Rust; backup identifiers are constrained. Files are created with `create_new(true)` and synchronized before returning success.
6. Restoring a backup uses the same rollback-on-failure transaction path as normal writes. JSON files are limited to 32 MiB by existing state reader. Normal saves retain previous-state `.bak` recovery.
7. Browser preview adapters implement identical command names for UI testing, but are **not** the Rust implementation. System telemetry is blank in browser preview, not fabricated.

## Risk register

- **Windows build unverified:** Cargo/Rust compiler is not installed in the development container. All code must pass Windows CI and native smoke tests before claiming release readiness.
- **System telemetry unverified:** `sysinfo` integration checked against v0.33.1 reference docs, not Windows runtime.
- **Backup portability:** App-internal backups are not yet user-exportable and are not encrypted. Provide export/import, checksums and versioned migration before production.
- **Reminders aren't OS notifications:** app must be open/visible. Sleep/hibernation and notification interception are not implemented.
- **Calendar isn't Outlook/Google Calendar:** only local events are stored, no ICS recurring events or external synchronization.
- **Accessibility/i18n:** UI is presently English, while user target is Indonesia; screen reader and keyboard test coverage remains limited.
- **All features from Poni aren't parity-verified:** Phase 3 adds independent implementations, not a confirmed binary clone.

## Next high-value stages

Windows native compilation & signing, native media controls, tray / autostart & multi-monitor positioning, Windows toast notifications, advanced local calendar with ICS import/export, opt-in dock / clipboard, versioned backup/export, local SQLite migration, accessibility, and performance tests.

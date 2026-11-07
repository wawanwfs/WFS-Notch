# WFS Notch · Phase 2 Bug Audit

| Bug ID | Existing problem | Correction | Test evidence | Native Windows state |
|---|---|---|---|---|
| B-01 | Async note saves could resolve out of order; older response could overwrite newer UI data | Serialized IPC mutation pipeline and save-flight coordination | Browser smoke: switching notes/Save now | Unverified |
| B-02 | Pending note draft could be lost on tab switch, collapse or Exit | `flushNote` before switch/collapse/exit; failed save prevents switch/exit | Browser smoke: note lifecycle; source inspection | Unverified |
| B-03 | Notes could appear to retain text from a different selected note during focus | Editor bound to selected note ID; new selections rebind values | Browser smoke: two notes and switching | Unverified |
| B-04 | Timer reset after restart; paused state ambiguous if pause clicked immediately | Rust-persisted timestamp state and explicit `paused` flag | Browser smoke: immediate pause/resume and reset; Rust unit tests defined | Unverified |
| B-05 | Hover setting existed but had no effect; no stay-open setting | New Rust commands to save toggles and `pin_open` behavior | Browser smoke: settings | Unverified |
| B-06 | No Exit UI on invisible-taskbar window | Explicit native close command and button | Browser smoke verifies preview limitation only | Unverified |
| B-07 | Corrupt primary JSON blocked startup even when backup valid | Backup recovery, preserve damaged JSON, no destructive silent reset | Rust tests defined only | Unverified |
| B-08 | Primary state disappeared after interrupted rename without recovery | Backup restore when primary missing | Source inspection; requires Windows fault-injection | Unverified |
| B-09 | Settings panel content clipped after added options | Scrollable active panel and visible keyboard focus styles | Browser visual smoke | Unverified |
| B-10 | No original, consistent app icon | Source SVG logo; regenerated 32/128/256/512 PNG and ICO; new wordmark | File/asset static audit | Icon bundle unverified |

### Remaining risks

- Not yet compiled with Rust MSVC; actual Tauri 2 Window API compatibility is unverified.
- Crash-consistent file replacement uses two filenames, not an atomic transaction across all Windows crashes/power loss scenarios. Test with induced I/O failures and migrate to SQLite for production.
- Browser UI tests use an in-memory localStorage adapter, not the Rust IPC bridge.
- No native Windows reference app functional comparison has been performed for this revision.
- Windows accessibility, 125–300% DPI, multiple monitors and native window hover transition must be verified.
- Other Poni-adjacent module categories are intentionally not claimed as complete.

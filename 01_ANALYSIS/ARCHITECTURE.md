# Target Architecture — WFS Notch (Rust)

```text
Windows 10/11 / user session
  |
  +-- Tauri 2 Windows (topmost transparent / WebView2)
  |     +-- Compact notch / expandable workspace
  |     +-- HTML/CSS/JS presentation layer (original implementation)
  |     +-- Tauri IPC commands (typed, minimally scoped)
  |
  +-- Rust core
  |     +-- user-facing command dispatcher
  |     +-- local storage / migrations / backup and recovery
  |     +-- scheduler, domain services, event bus
  |     +-- media session / Windows notifications adapters
  |     +-- permission and consent manager
  |     +-- AI provider adapters (opt-in)
  |
  +-- Windows adapters
  |     +-- WinRT/Win32 media, network, power, window management
  |     +-- OS-level notifications and system monitoring
  |     +-- multi-monitor, DPI, hotkey, focus behavior
  |
  +-- User data: %APPDATA%/id.wfs.notch (Tauri app_data_dir)
```

## Phase 1 implementation

- Rust owns persisted todo/note/settings state, serialized to JSON under Tauri's app data directory.
- UI is a bundled static asset. There is **no external CDN** or hosted JavaScript at runtime.
- The browser preview's localStorage adapter is for visualization only; native mode invokes Rust commands.
- A JSON backup is kept on writes. Corrupt state errors deliberately stop startup instead of overwriting user data.
- AI, network, screen monitoring, notifications and clipboard listeners are **not** enabled in this starter.

## For the production build

- Migrate storage to SQLite (e.g. rusqlite with WAL) for concurrent events, transactional safety, versioned migrations and search.
- Split domains into crates: `wfs-core`, `wfs-storage`, `wfs-win32`, `wfs-media`, `wfs-ai`, `wfs-notifications`, `wfs-updater`, `wfs-ui-host`.
- Define explicit permission prompts for microphone, location-sensitive Wi-Fi, calendar links, clipboard history, and local AI CLI data.
- Encrypt API credentials using Windows Credential Manager / DPAPI; never plain JSON.
- Use on-device-first behavior; opt-in telemetry disabled by default.
- All integration APIs must implement graceful degradation if Windows build, drivers or permissions differ.
- For OS hooks / overlays: test DPI, multiple monitors, fullscreen apps, gaming anti-cheat environments, accessibility, screen readers and touch.
- For updater: sign release manifests, hash and signer verification, staged update and rollback, no arbitrary remote EXE execution.

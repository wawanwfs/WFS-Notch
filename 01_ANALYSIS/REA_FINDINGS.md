# REA Findings — Poni 1.5.0

Date: 2026-11-07. Method: **static, read-only binary inspection** plus independently published user documentation. Neither EXE was executed and no Windows runtime behavior was measured.

## Binary fingerprints

- Portable: **PE32+ / AMD64**, 11,895,808 bytes, 6 sections; SHA-256 `4c0ee8719d474eed32389352d41874ab11b1541272a3886f63f3ff5cc62388ce`.
- Installer: **PE32 / x86**, 4,973,625 bytes, 5 sections; SHA-256 `e45f95d75e47d4869321e64507d2999931868f9c3971e8681ec9277007a5fdde`. NSIS installer wrapper; this is not evidence of the application runtime being 32-bit.
- Portable contains symbol/debug-path strings that explicitly identify **Rust**, `tauri-2.12.4`, `tauri-runtime-wry-2.12.2`, `tauri-plugin-updater-2.14.0`, and `tokio-1.54.0`.
- Portable imports Windows APIs/libraries including `user32.dll`, `dwmapi.dll`, `wlanapi.dll`, `d3d11.dll`, `dcomp.dll`, `pdh.dll`, `bcrypt.dll`, `ws2_32.dll`. Imports are a capability clue, not evidence every feature is fully implemented.
- The binary includes references to `__TAURI_INTERNALS__`, WebView2, and user-facing phrases related to Claude Code, Codex, and translations. No proprietary code or bundled app assets have been carried into this new project.

## Key conclusions

1. **Confirmed architecture family:** Rust + Tauri v2 / WRY / WebView2. Strong evidence from embedded dependency paths in the submitted portable binary.
2. **Installer format:** NSIS. Observed by PE signature identification, not executable behavior.
3. **Original UI design and runtime semantics:** not yet reverse engineered through visual or dynamic testing.
4. **Potential integration points:** Windows system APIs, music/media session, network, privacy-safe local data, AI connectors, and system overlays. Exact protocols and data structures require targeted validation.
5. **No proof of 100% feature parity:** a genuine parity claim requires Windows black-box comparison, integration tests, edge cases, and accessibility validation.

## External references (public documentation)

- Official product page: https://poni.web.id/
- Official guide: https://poni.web.id/panduan/
- Official v1.5.0 release notes: https://poni.web.id/catatan-rilis/
- Project public repository: https://github.com/liimalim/Poni

The public repository as viewed on 2026-11-07 showed public-facing resources and translations, not a full application source tree. Do **not** assume that a public repository or downloadable EXE grants permission to copy proprietary source, branding, UI artwork, or media assets. Reimplement functionality independently.

## Evidence policy

- `BINARY`: direct observation from the files supplied by the user.
- `DOCS`: explicitly described by the official product documentation.
- `DESIGNED`: intended functionality of WFS Notch (new implementation).
- `TESTED`: only claim when executable tests actually run successfully on a supported Windows machine.
- `UNVERIFIED`: not yet inspected or validated.

## Current implementation coverage

Phase 1 Rust/Tauri source includes a compact/expanded notch window, original UI, module navigation, persistent to-do CRUD, notes CRUD, accent settings, and an in-process focus timer. The timer does not yet persist across restarts. This package **has not been compiled or executed on Windows in this environment** because Rust/Cargo and a Windows runtime are unavailable here. JavaScript syntax and JSON configuration were checked.

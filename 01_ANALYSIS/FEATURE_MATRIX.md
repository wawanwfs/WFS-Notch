# Feature Parity Matrix — requirements inventory, not completion report

Legend: `P1/2 implementation source (not native Windows-verified)` = Rust Tauri or UI code included, not Windows-verified; `Backlog` = architecture / tests needed; `DOCS` = official published user guide.

| Domain | Behaviors to replicate independently | Evidence | Status |
|---|---|---|---|
| Window shell | Compact top-center overlay, hover expand, collapse, focus | DOCS/BINARY | P1/2 implementation source (not native Windows-verified) |
| Main navigation | Modules with active tab and settings | DOCS | P1/2 implementation source (not native Windows-verified) |
| Data persistence | Local settings, todo & notes; recovery on write failure | DOCS/DESIGNED | P1/2 implementation source (not native Windows-verified) |
| Todo | Add/toggle/delete, completed count | DOCS | P1/2 implementation source (not native Windows-verified) (basic) |
| Notes | Create/edit/autosave/delete | DOCS | P1/2 implementation source (not native Windows-verified) (basic) |
| Focus timer | Presets, start/pause/reset | DOCS | P1/2 implementation source (not native Windows-verified) (in memory) |
| Media sessions | Playback controls, cover art, progress, audio glow | DOCS | Backlog |
| Lyrics | Synced lyrics, timing adjustment, transliteration | DOCS | Backlog |
| Calendar | iCal/ICS ingestion, today/week/month, meeting links | DOCS | Phase 3: local event CRUD only; advanced ICS/sync backlog |
| Reminders | Schedule, snooze, missed/repeating task handling | DOCS | Phase 3: app-visible event reminder only; native toast and snooze backlog |
| Windows notifications | Toast intercept, history, quiet hours, privacy | DOCS | Backlog |
| Dock | App icons, window groups, previews, multi-monitor | DOCS | Backlog |
| System dashboard | CPU/RAM/GPU/battery/performance counters | DOCS | Phase 3: Rust CPU/RAM source only, Windows test pending; GPU/battery backlog |
| Quick control | Volume, brightness, output/microphone selection | DOCS | Backlog |
| Wi-Fi / Bluetooth | List/connect/change radio state, permissions | DOCS | Backlog |
| File shelf | Drop zone, compression, image conversion, move/share | DOCS | Backlog |
| Phone-to-Shelf | QR/LAN file/text exchange, server auth, expiry | DOCS | Backlog |
| Clipboard history | Optional, local encryption, sensitive-data filtering | DOCS/DESIGNED | Backlog |
| Window snap | Window-target drag zone and deterministic tiling | DOCS | Backlog |
| Weather | Official provider APIs, caching & offline fallback | DOCS | Backlog |
| Prayer times | Provider/location options, countdown and reminders | DOCS | Backlog |
| Pomodoro / stopwatch | Multiple timing modes & pause behavior | DOCS | Backlog |
| Health breaks | Hydration, breathing, eye-break reminders | DOCS | Backlog |
| Day/screen time | Period tracking, charts and per-app breakdown | DOCS | Backlog |
| AI Chat | Provider configuration, history, optional tool calls | DOCS | Backlog |
| AI usage | Consent-based Claude/Codex usage limits | DOCS | Backlog |
| Claude Code events | Consent-based prompts & done notices | DOCS | Backlog |
| Translate | User-selected model and language targets | DOCS | Backlog |
| GitHub | Authentication and pending PR review list | DOCS | Backlog |
| Buddy | Optional draggable animated character | DOCS | Backlog |
| Animation | Configurable notch effects, reduced-motion support | DOCS/DESIGNED | Backlog |
| Themes | Glass/frosted/color variants; accessibility | DOCS | Backlog |
| Auto-update | Release signing, manifest validation, rollback | DOCS | Backlog |
| i18n | Indonesian & English, scalable translation architecture | DOCS/DESIGNED | Backlog |
| Backup/restore | Versioned, checked, migration-safe local exports | DOCS | Phase 3: app-local snapshot/restore; portable exports/backups backlog |
| Security | Consent for background sensors, least privilege, no default remote telemetry | DESIGNED | Backlog |
| Windows installer | NSIS, per-user installation, signer trust | DOCS/BINARY | Skeleton config only |

## Definition of Done per feature

1. Requirement documented, including settings, data formats, error handling and accessibility.
2. Windows reference observation or official API/specification with recording/test evidence.
3. Independently authored Rust service and frontend integration.
4. Unit tests, integration tests, and at least one end-to-end Windows workflow.
5. Parity score and outstanding behavioral differences reported honestly.
6. No copied proprietary resources and no secrets embedded in the released app.

### Phase 2 follow-up

Read `BUG_AUDIT_PHASE2.md` and `06_EVIDENCE/TEST_REPORT.md` for corrections and actual test evidence. The existing broad feature matrix deliberately remains mostly Backlog.

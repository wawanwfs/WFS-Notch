# WFS Notch Windows parity acceptance

Compare WFS Notch with **Poni 1.5.0** on Windows 10 and Windows 11. Record each result separately. CI builds and browser screenshots do not count as native or comparative proof. Use test accounts, sample events, and sanitized screenshots.

## Before testing

1. Record the WFS commit and Actions run, package SHA-256, exact Windows build, WebView2 version, display layout and DPI. Use the unsigned NSIS or portable artifact linked to that source commit.
2. Back up WFS data and use a test profile for installation and upgrade. Keep production notes and notifications out of the evidence.
3. Test the same scenario in each application on the same machine. Record differences as `PASS`, `FAIL`, `NOT TESTED`, or `INTENTIONAL DIFFERENCE`, with observed output and reproduction steps.

## Scenario matrix

| Area | Procedure | Expected WFS behavior |
| --- | --- | --- |
| Display | Configure 1, 2 and 3 monitors. Move the primary, use negative coordinates, change 100/125/150% DPI and unplug/replug the selected display. | Notch geometry stays usable and no stale output is presented as the selected display. |
| Focus and fullscreen | Test maximized, borderless and exclusive fullscreen, then RDP reconnect. Exercise pin, always-on-top, opening and closing. | No unwanted focus steal and correct visibility with the configured policy. |
| Media | Test Spotify, two browser sessions and Windows Media Player. Change track, seek, disconnect and reconnect a player. | Metadata and controls belong to the selected player. Ambiguous matches are refused. |
| Notifications | Test opt-out and opt-in, Focus Assist, lock, suspend and restart. Restore an older backup after a reminder. | No notification without consent and no replay of an already reserved reminder. |
| Calendar | Create a backup, apply a matching scoped ICS cancellation and restore the old backup. Then explicitly reset cancellation history and restore a snapshot from before the reset. | Active canceled events remain absent until intentionally reset. An old snapshot does not reinstate cleared markers. |
| Tasks | Use `Asia/Jakarta` and `America/New_York`. Complete a recurring task around spring-forward and fall-back. Reload and reopen the editor. | The deadline and displayed timezone agree, skipped local times do not create duplicates, and overlaps choose the earlier occurrence. |
| Accessibility and visuals | Use keyboard navigation and Narrator through all 16 modules. Compare collapsed and expanded surfaces at supported scaling. | All interactive controls are reachable, named and readable. Record intentional WFS styling differences separately. |
| Packaging | Test clean install, portable launch, upgrade from the previous version and uninstall in the test profile. | Upgrade preserves data and recovery restores the expected state. |

To capture a local report and package hashes after testing, run `.\scripts\phase16-native-acceptance.ps1 -Interactive` from the repository root. A self-reported result is not independently verified. Do not upload personal content or logs without review.

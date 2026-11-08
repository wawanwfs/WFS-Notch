# Development roadmap

Baseline: v0.9.0, Windows Rust + Tauri 2. Current release: v0.10.0 development.

## Next priorities
1. Consolidate source layout as a product, without numbered folders or research notes.
2. Opt-in native Windows toast notifications **from WFS Notch itself**, default off; no OS notification scraping.
3. More reliable Windows GSMTC playback session selection and explicit transport controls; album artwork remains pending.
4. Real fullscreen, multi-monitor disconnect/reconnect and mixed-DPI testing on Windows 10/11.
5. Polish spacing, hover/transition timing, accessibility and visual screenshots.

## Progress measurement

Reference feature ledger contains 38 feature groups. The v0.9.0 baseline has 3 implemented, 15 partial and 20 gaps. Weighted development estimate: **(3 + 15×0.5) / 38 = 27.6%**. This *planning score* is not a measure of visual similarity, production readiness, runtime coverage, or official feature equivalence.

## Completion criteria

All CI checks PASS; Windows native smoke tests; privacy/accessibility review; change review; reproducible installer; original WFS assets only. Do not assert 100% parity without independent behavioral evidence.

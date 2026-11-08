# Contributing to WFS Notch

This project accepts changes through reviewed branches and GitHub pull requests.

1. Keep the product brand original and do not commit third-party binaries, decompilation outputs or private user files.
2. Put client functionality under `src/features/`, styles under `src/styles/`, and Windows/Rust code under `src-tauri/src/`.
3. Add tests for each behavioral change. Run `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and the browser regression suite.
4. Never present a passing Windows compile as proof of working hardware integration or 100% feature parity.
5. Do not introduce global remote tunnels, credentials, unsigned binary downloads or OS-level permissions without specific review.

The project currently has no public reuse license. Please contact the owner before redistribution.

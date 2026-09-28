# Development and validation

Date: 2026-09-28

Status: current commands and test policy. Tags: #development #testing

Rust is pinned by `rust-toolchain.toml` to installed toolchain 1.97.1. The host's global
1.85 default is unchanged. Commit Cargo.lock for reproducible application dependencies.
Ratatui + Crossterm form the TUI; rusb uses vendored libusb so the scaffold does not
require system libusb development headers. Building still needs a C compiler/linker.

```sh
cargo run --locked -- --help
cargo run --locked -- doctor
cargo run --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release
python3 scripts/check-terminal.py
```

## Test classes

- Default: fast address/encoding contract checks, target-screen rendering, undersized UI.
- Default Linux PTY checks: exit keys, terminal restoration and absence of mouse/clipboard capture.
- Add normal safety, recovery, schema, routing and persistence regressions as those paths exist.
- Physical fixture tests are manual and pending; never run from CI.
- Historical algorithm comparisons, recordings, exhaustive matrices and long benchmarks
  belong in opt-in tests once introduced, with documented commands and saved evidence.
  None exist in this scaffold.
- Run the complete normal suite for shared models, rendering, output, concurrency or safety
  changes. Use focused tests while iterating; do not ask the user to classify tests.

GitHub Actions runs formatting, Clippy, default tests, a release build and PTY checks
on Linux for pushes and pull requests. See [CI details](../../contrib/ci/README.md).
The local build provides Raspberry Pi/AArch64 verification; CI does not emulate fixtures.
No release binaries are published automatically. No distribution license has been chosen.

Related: [terminal contract](0007-terminal-contract.md), [roadmap](0010-roadmap.md).

[Notebook index](../index.md)

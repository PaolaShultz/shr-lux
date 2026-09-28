# shr-lux

Autonomous, source-aware DMX lighting for small live bands. Rust on Raspberry Pi,
with an 80×25 TUI and planned MIDI control/LED feedback.

**Status: initial scaffold. No audio analysis, MIDI handling or DMX output yet.**
The connected uDMX dongle responds to USB descriptor queries; physical lighting output
has not been tested. Read [hardware evidence](docs/notes/0002-hardware-baseline.md).

## Run

Requires Rust/rustup and a C build toolchain. The repository selects Rust 1.97.1.

```sh
cargo run --locked -- --help
cargo run --locked -- doctor
cargo run --locked
```

The TUI uses q, Esc or Ctrl+C to quit. It does not capture the mouse, manipulate the
clipboard or open links on the host. `doctor` only reads USB descriptors; an access-denied
message is explained in [USB permissions](docs/notes/0004-linux-usb-access.md).

## Reference

- [Project notebook](docs/index.md) — zk notes, protocol references and decisions
- [Original idea](idea.md) — preserved concept specification
- [Roadmap](docs/notes/0010-roadmap.md)
- [First fixture test](docs/notes/0011-fixture-bench-test.md)
- [Development and checks](docs/notes/0012-development-and-validation.md)

## Layout

```text
src/          CLI, TUI, USB diagnostics, pure DMX contracts
contrib/      Optional udev rule and CI documentation
.github/      Automatic build and test workflow
docs/         Standalone zk notebook
idea.md       Original concept
```

Run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`
and `cargo test --locked --all-targets` before committing. Build with `cargo build --locked --release`, then run
`python3 scripts/check-terminal.py` for normal Linux terminal recovery checks.
Hardware tests are manual.
This is a private project; distribution licensing is undecided.

GitHub Actions runs the normal build and test checks on pushes and pull requests.
See [CI details](contrib/ci/README.md).

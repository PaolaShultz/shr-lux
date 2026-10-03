#!/bin/sh
# Normal production checks; run from the repository root.
set -eu
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
python3 scripts/check-simulation.py
cargo build --locked --release
python3 scripts/check-terminal.py

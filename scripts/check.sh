#!/bin/sh
# Normal production checks; run from the repository root.
set -eu
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release
python3 scripts/check-terminal.py

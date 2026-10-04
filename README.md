# SHR Lux

An experimental lighting engine for small live bands, built in Rust for
Raspberry Pi. Analyze separate instrument recordings and preview coordinated
scenes in an 80×25 terminal or on MiniLab mkII pads.

**Status: working recorded-audio simulation with source activity, kick candidates,
kick pulse estimates, automatic scenes and optional MiniLab mkII pad output.
Live audio capture, MIDI input controls and DMX output remain pending.**
Integrated software additionally provides a private null-output lighting authority
with programmer/Hold/cue/playback state, timed release and disarmed checkpoint
recovery (LX-01–04). LX-05 adds an explicit named analysis subscription, ASSIST
proposals and bounded intensity AUTO grants; device-free software validation passed and is recorded
in [the owning plan](docs/notes/0027-gigpies-implementation.md) and
[analysis protocol/evidence](docs/notes/0032-analysis-subscription.md).

Minimum design target: Raspberry Pi 1 with 512 MB; hardware validation pending.
The connected uDMX dongle responds to USB descriptor queries; physical lighting output
has not been tested. Read [hardware evidence](docs/notes/0002-hardware-baseline.md).

## Run

Requires Rust/rustup and a C build toolchain. On Linux install `libasound2-dev` and
`pkg-config`. The repository selects Rust 1.97.1. Optional listening uses SoX `play`.

```sh
git clone https://github.com/PaolaShultz/shr-lux.git
cd shr-lux
cargo run --locked -- --help
cargo run --locked -- doctor
cargo run --locked
```

The TUI uses q, Esc or Ctrl+C to quit. It does not capture the mouse, manipulate the
clipboard or open links on the host. `doctor` only reads USB descriptors; an access-denied
message is explained in [USB permissions](docs/notes/0004-linux-usb-access.md).

## Preview musical programs

```sh
cargo run --locked --example show-preview -- punk
cargo run --locked --example show-preview -- metal --bursts
cargo run --locked --example show-preview -- atmospheric
```

These print a synthetic timeline without waiting or accessing hardware. Programs hold
scenes, suggest fade times, and optionally request bounded accents. They do not listen
to music or generate lighting output. See the [music research](docs/notes/0015-musical-direction.md)
and [Pi 1 feasibility plan](docs/notes/0014-pi1-feasibility.md).

## Simulate four AUX inputs with real songs

Two local sessions are prepared: The Complainiacs’ **Etc** (punk) and Dark Ride’s
**Hammer Down** (metal). Each supplies kick, bass and two guitar channels.

```sh
python3 scripts/prepare-simulation.py --download
cargo run --locked --release --example aux-replay -- recordings/simulation/punk/aux.wav --realtime
```

No sound input is needed. Audio stays in ignored `recordings/` for local
educational use. See the [simulation guide](docs/notes/0017-local-aux-simulation.md)
for preparation, routing and source terms.

### Analyze music and preview the show

```sh
cargo build --locked --release
target/release/shr-lux simulate punk
target/release/shr-lux simulate metal --midi --listen --bursts
# A short passage, without sound playback or a terminal UI:
target/release/shr-lux simulate metal --midi --headless --start 258 --seconds 20 --bursts
# Static four-second test on the connected MiniLab mkII:
target/release/shr-lux pads-test
# Separate eight-second irregular white-strobe audition:
target/release/shr-lux strobe-test
# Offline metrics (no hardware access):
mkdir -p local
target/release/shr-lux analyze metal --bursts > local/metal-analysis.csv
```

`--midi` explicitly enables pad output; omit it for terminal-only development.
`--listen` plays the prepared stereo monitor through the default sound output;
playback and analysis have independent clocks, so listening is approximate.
Use `--listen` only after configuring your sound output.
Keys: **q/Esc/Ctrl+C** quit, **b** toggles burst requests, **x** toggles blackout.

All eight pads form a coordinated show: mirrored sweeps, opposing chases, guitar
conversation and center blooms. Bass shapes color spread; kicks add paired accents.
Qualified bursts produce three short flashes across mirrored pairs while the base
colors remain. Both banks display the same show. MiniLab colors are discrete, so
smooth RGB fades become color steps. No preset is stored or changed.
See [show design and audition](docs/notes/0019-composed-pad-show.md).

The analyzer measures isolated sources; it does not yet identify song sections,
downbeats, harmony or solos. See [implementation and validation](docs/notes/0018-analysis-and-pad-preview.md)
for measured results, shutdown behavior and remaining limits.

## Lighting design research

The [design study](docs/notes/0020-lighting-design-study.md) connects stage composition,
color science, concert designers' practice and musical timing to a proposed small
engine. It includes annotated sources, a code audit and controlled audition exercises.
These proposals are separate from the current runtime. Hardware output is enabled
only by explicit commands or flags; see the linked test procedures before use.

## Reference

- [Project notebook](docs/index.md) — zk notes, protocol references and decisions
- [Original idea](idea.md) — preserved concept specification
- [Roadmap](docs/notes/0010-roadmap.md)
- [First fixture test](docs/notes/0011-fixture-bench-test.md)
- [Development and checks](docs/notes/0012-development-and-validation.md)

## Layout

```text
src/          CLI, TUI, USB diagnostics, pure DMX contracts, show policy, replay, analysis, pad output
contrib/      Optional udev rule and CI documentation
.github/      Automatic build and test workflow
docs/         Standalone zk notebook
idea.md       Original concept
```

Run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`
and `cargo test --locked --all-targets` before committing. Build with `cargo build --locked --release`, then run
`python3 scripts/check-terminal.py` for normal Linux terminal recovery checks.
Fast preparation checks: `python3 scripts/check-simulation.py`.
Hardware tests are manual.

GitHub Actions runs the normal build and test checks on pushes and pull requests.
See [CI details](contrib/ci/README.md).

## License

No project license has been selected. Dependency licenses and the terms for
external recordings are documented separately in the linked guides.

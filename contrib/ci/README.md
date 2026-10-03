# GitHub Actions

The active workflow is [.github/workflows/ci.yml](../../.github/workflows/ci.yml).
It runs formatting, Clippy, normal Rust tests, a release build and Linux PTY recovery
checks on pushes and pull requests. It needs no lighting hardware and sends no DMX.

Local equivalent: `sh scripts/check.sh` from the repository root.

Workflow upload was initially blocked by a missing OAuth scope. Authorization was
completed on 2026-09-28, and the workflow was moved from its template location into
GitHub's active workflows directory. Credentials remain outside the repository.

The fast preparation contract checks (`python3 scripts/check-simulation.py`) use
synthetic PCM and require no SoX, downloads or private media. Full-song simulation
audits are opt-in and remain outside CI.

Linux builds install `libasound2-dev` and `pkg-config` for the optional runtime MIDI
output. Normal tests do not open MIDI devices.

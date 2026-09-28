# GitHub Actions

The active workflow is [.github/workflows/ci.yml](../../.github/workflows/ci.yml).
It runs formatting, Clippy, normal Rust tests, a release build and Linux PTY recovery
checks on pushes and pull requests. It needs no lighting hardware and sends no DMX.

Local equivalent: `sh scripts/check.sh` from the repository root.

Workflow upload was initially blocked by a missing OAuth scope. Authorization was
completed on 2026-09-28, and the workflow was moved from its template location into
GitHub's active workflows directory. Credentials remain outside the repository.

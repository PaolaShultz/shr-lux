# Enable GitHub Actions

The current GitHub CLI token has repository access but lacks the `workflow` scope.
GitHub rejected the initial push containing `.github/workflows/ci.yml`. The workflow
is therefore stored here as an inactive template; local validation has passed.

When an account credential with workflow permission is available, copy
`contrib/ci/github-actions.yml` to `.github/workflows/ci.yml`, commit and push.
For GitHub CLI OAuth authentication, `gh auth refresh -h github.com -s workflow`
starts the interactive authorization flow. No token values belong in the repository.

Local equivalent: `sh scripts/check.sh`. This includes the normal Rust tests and fast
Linux terminal checks; no fixture is needed and no DMX values are sent.

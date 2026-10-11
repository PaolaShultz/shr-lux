# Opt-in musical owner corpus

Owner-produced synthetic JSON, with no private audio. The two binding schemas share
musical estimate semantics: `configured` uses the explicit ordered binding and
`legacy-binding` uses the original input mapping. These small snapshots exercise
tracking, exact latest spectral-hop provenance and stale-reset behavior.

`configured-estimates` and `legacy-estimates` additionally retain real harmonic
and arrangement estimates from a deterministic C/F/G/C progression. Their final
active state contains chord, C-major key/Camelot, harmonic movement and a tentative
section estimate. Stale and unknown states clear all musical evidence; human Hold
remains independent. These are software examples, not recognition-quality evidence
from a live band or hardware qualification.

Refresh small fixtures explicitly:

```sh
CARGO_INCREMENTAL=0 LUX_GENERATE_MUSICAL_CORPUS=1 cargo test --locked -j1 --lib musical_opt_in_schema_source_frames_and_stale_reset
```

Refresh the longer evidence renderer explicitly (ignored in the normal suite):

```sh
CARGO_INCREMENTAL=0 cargo test --locked -j1 --lib musical_owner_estimate_corpus -- --ignored
```

Review the resulting JSON diff before accepting it. The longer renderer also checks
AUTO cap/provenance, independent manual900/Hold900 and estimate reset. Consumer
regressions read the retained files without rendering PCM.

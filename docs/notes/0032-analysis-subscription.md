# Independent named analysis subscription

LX05 is implemented and independently accepted with matching release and
device-free process validation. Console integration is tracked centrally.
[Owning plan](0027-gigpies-implementation.md) · [Prior provider](0031-local-provider-integration.md).

The root-accepted GP04-local:1 / C-ANALYSIS:1 input describes explicit stream3,
48kHz, kick/bass/guitar-1/guitar-2, input-01..04, raw-pre-fader mapping,
source epoch/map/calibration revisions, and oldest first-packet acquisition time
on the same-host Linux monotonic clock. Lux decodes signed PCM24 independently;
it imports no provider DSP or sibling dependency. The exact accepted synthetic
[E09 data](../../tests/fixtures/lx05/v1/README.md) is retained for regression checks.
44.1kHz, unknown maps and fields, malformed/reserved bytes, future or >100ms-old
acquisition times, gaps, overlaps, reorder, loss and identity changes refuse input.
Missing windows never become zero-filled musical silence.

The first delivered window must begin on a whole-window offset from the
descriptor's 48-frame-aligned source origin, with the corresponding GPA1 sequence.
Late attachment may skip whole windows; it cannot reset sequence or relabel a
partial window. Epoch and mapping/calibration revisions must be nonzero.

Detach and reattachment clear current source-window, age and calibration-count
metadata. A new descriptor cannot inherit a previous source's observations, and
an absent source carries no window range. Retained automatic lighting contributions
keep their independent original provenance while manual control remains available.

A separate transport worker has a two-window inbox, 100ms total frame deadline,
250ms reconnect backoff and bounded shutdown. It opens only the explicitly
configured same-UID private Unix socket. No audio/MIDI/DMX device discovery,
playback, display, remote production control or provider algorithm is involved.
A fresh validated descriptor does not authorize analysis or automation. Explicit
soundcheck uses the existing Calibrator, at least50 contiguous10ms windows,
all four sources observed, maximum10seconds. The existing Analyzer then settles
for500ms before any ready feature claim. Beat/downbeat/harmony remain unavailable.

The opt-in lx05-v1 wire extension and AUTO continuity/grant representation
were root/independently reviewed; the exact accepted consumer corpus is frozen.
Legacy schemas and corpora must retain their accepted behavior. Physical output
remains null/disarmed and unknown. No hardware or combined-load claim follows.

Independent review identified corrections for acquisition age
crossing100ms during DSP/encoding, a potentially blocking Unix connect under full
backlog, and automatic snapshot-budget growth. The client now uses nonblocking
connect with100ms deadline/cancellation. Authority updates validate a private
candidate, reserve one encoded page for safe freeze diagnostics, and re-sample
source age after DSP and before committing. Focused backlog, processing-delay/
clock regression and deterministic near-budget checks passed. The final expiry
check also clears ASSIST proposals without an AUTO grant after encoding crosses
the100ms source-age limit. These are software
invariants only, not scheduler/physical safety evidence.

## Explicit synthetic process reproduction

Build the owning release using the pinned toolchain, locked dependencies, normal
profiles and shared parent-held build helper. Use the exact independently accepted
GP04 `gigpies-headless` executable, not a moving sibling target. Run:

```sh
python3 scripts/check-lux-analysis.py \
  --provider /absolute/accepted/gigpies-headless \
  --lux target/release/lux-service \
  --evidence /absolute/private/lux-analysis-evidence.json
```

The bounded harness creates only owned0700 temporary directories, starts the
explicit fouraux/analysis provider and opt-in Lux, uses actual C-LIGHT commands
for fresh calibration and AUTO grant, checks real positive feature energy,
kills/waits only its source child, checks retained AUTO provenance/value, and
proves human touch900 still works. It joins all own children and removes own
synthetic socket/state scratch. No media is generated or retained in Git.
The matching corrected-source release process check passed, as did103normal Rust
tests, warning-denied Clippy, fmt,8PTY,6fast simulation checks, legacy corpus
validation and actual release/checkpoint/restart checks. See the owning plan for
commands/counts. Exact artifact/evidence hashes remain in the private handoff;
physical and combined-load acceptance remain separate.

### Analysis without timed release

A final consumer check also exercised the accepted release with `--analysis` alone.
Its actual LX05 inventory advertises analysis while omitting release/checkpoint;
this valid combination is captured in `tests/fixtures/lx05/v1/untimed-absent.json`.
The wire note now states that analysis and the explicit timed/durable modes are
independent. Consumers must validate advertised capabilities before timed preview
or checkpoint operations. `--durable` includes timing, so it replaces `--timed`.
Provider code and the original command/feature corpus are unchanged. The existing
full normal/style/release evidence is reused for this documentation/fixture change;
the additional actual process check joined its child before removing its private
socket directory. This remains synthetic null-output software acceptance.

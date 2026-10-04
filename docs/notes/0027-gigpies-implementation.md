# GigPies lighting authority implementation

Historical planning baseline **2026-10-04 / GP-2026-10-04.1**. The original task
table and launch prompt below preserve the planning record; current software state
is listed here and in the dated progress entries. [Central inventory](https://github.com/PaolaShultz/gigpies/blob/main/docs/MODULE_IMPLEMENTATION_MAP.md) ·
[Agreed contracts](https://github.com/PaolaShultz/gigpies/blob/main/docs/MODULE_CONTRACTS.md). Existing product roadmaps remain authoritative for
unrelated work; this plan owns only the GigPies integration increments below.

## Current implementation state — task0009

| Task | Current software state | Remaining boundary |
|---|---|---|
| LX-01 | Accepted synthetic fixture/patch and snapshot contracts | Actual fixture maps unverified |
| LX-02 | Accepted static programmer/Hold/cue/playback authority | Null output only |
| LX-03 | Accepted bounded private service and writer grants | Production remote control unavailable |
| LX-04 | Accepted timed release and durable disarmed recovery | Physical recovery unverified |
| LX-05 | Independently accepted named GP04 analysis, ASSIST and scoped AUTO; matching release and device-free checks passed | Physical/load acceptance remains separate; consumer integration is tracked centrally |
| LX-06 | Deferred | Requires separate fixture/output authorization |

## Objective and boundary

Own lighting analysis, patch/capabilities, manual programmer/holds, cues, per-attribute source arbitration, timing and output. Retain standalone terminal/pad preview as separate modes. Integrated engine mode claims neither Desk controller nor LED output. Preserve idea.md and the existing artistic research; do not copy unlicensed Lux code into a surface.

## Source and evidence reviewed

Repository: `/home/shome/p/shr-lux`. Inspected HEAD: `ba4ccd92656e6d2a3cbc6424cdc2a017d3e4f14d`.
Clean at inspection; recheck before editing. This dated observation is not a future ownership claim.

Owning documents: README.md, idea.md, docs/index.md; notes/0005-architecture.md, 0006-dmx-fixtures.md, 0010-roadmap.md, 0012-development-and-validation.md, 0018-analysis-and-pad-preview.md, 0024-design-to-engine.md.

Source inspected: `src/dmx.rs::{Address,encode_range}`, `analysis.rs::{Analyzer,Snapshot}`, `show.rs::Director`, `preview.rs::Preview`, `simulation.rs`, `midi.rs`, `src/lib.rs`.

`analysis::Analyzer` uses four-source 480-frame windows; `show::Director`,
`preview::Preview`, recorded replay and opt-in MIDI pad output exist. `dmx::Address`
and `encode_range` validate a USB request but send no DMX. Notebook 0018/0019 owns
recorded-source/pad evidence; fixture programmer/cue authority, persistence and
physical DMX are missing. Lightdesk Simulator is not a Lux implementation.

These are source inspection and previously recorded results, not fresh builds or
physical acceptance. The planning session runs documentation checks only.

## Milestones and tasks

First: validated synthetic fixture/patch capability model and matching consumer corpus. Next: authoritative manual static look → null sink with explainable sources; framed service; timed release/durable restart; named audio feeds; actual fixtures only after a separate gate.

Task states are execution dependencies: READY has no missing software provider;
WAITING names its precise prerequisite; DEFERRED has an activation condition.
Source delivery and build reservation are additional launch prerequisites on a
peer. Every row has one owner, the repository named in its Owner column. A later
task starts only after the previous artifact is reviewed, never merely delivered.

| Task / priority / state | Owner | Work area, inputs and required artifact | Output and measurable acceptance |
|---|---|---|---|
| LX-01 / P0 / READY | SHR Lux | Add `src/fixture.rs`, `src/lighting_contract.rs`, tests and lib exports; reuse `dmx::Address`. Inputs C-LIGHT:1/E04 and common envelope; no surface or audio provider needed. | Versioned synthetic patch/capability validation, coherent RGB/position groups, overlap/default/range checks, bounded snapshot/source vocabulary and E04 fixture corpus. No output driver. Strong review for units and capability truthfulness. |
| LX-02 / P0 / WAITING | SHR Lux | LX-01 reviewed C-LIGHT:1 fixtures; new `src/authority.rs`, null sink. | Real static programmer/Hold/cue/palette/playback engine, HTP and activation order, manual zero, masters/blackout/mode continuity. Engine-owned E05 preview tokens; release commit unavailable until LX-04 supplies timing. One authoritative state, no copied surface engine. |
| LX-03 / P1 / WAITING | SHR Lux | LX-01/LX-02, GP-01 C-SHOW:1 fixtures, C-LIGHT:1 local framing. New bounded service adapter. | Read-only attach first, then validated grants/commands; stale/duplicate/slow client tests; integrated mode never opens MIDI output. Publish exact request/response corpus consumed by LD-03/04. |
| LX-04 / P1 / WAITING | SHR Lux | LX-02 and E05, persistence rules in C-SHOW/C-LIGHT. `authority.rs`, new timing/storage. | Injected 10 ms logical clock, 500 ms reviewed release from actual current value, bounded persistence of cue/holds/master/blackout, disarmed restart and cancelled transients. Part A timed intensity release, Part B durable state, each separately reviewed. Moving effects remain unavailable until explicit path/slew design review. |
| LX-05 / P1 / WAITING | SHR Lux | GP-04 C-ANALYSIS:1/E09 corpus, LX-04 for grants. `analysis.rs`, subscription adapter, show policy seam. | Map named four-source PCM to existing Analyzer, invalidate gaps/calibration changes, hold look on uncertain/lost input, ASSIST proposals and bounded AUTO without stealing human holds. Manual engine remains useful with analyzer absent. |
| LX-06 / P2 / DEFERRED | SHR Lux | LX-04, B-FIXTURE actual maps/loss policy, explicit physical reservation. `dmx.rs`, one output worker. | Known one-fixture low-level profile, explicit arm/rearm, bounded driver/errors, encoding/submission vs observation; then fixture-specific movement/color limits. No universal zero-universe fallback or implied strobe permission. |

## Validation and failure behavior

Focused new `cargo +1.97.1 test --locked --lib fixture -j 1` with CARGO_INCREMENTAL=0, then complete normal `cargo +1.97.1 test --locked --all-targets -j 1`; fmt and warning-denied all-target Clippy. Existing owner normal release/PTY path: `cargo +1.97.1 build --locked --release -j 1` then `python3 scripts/check-terminal.py`; fast preparation contracts `python3 scripts/check-simulation.py`. None uses --midi/--listen/pads-test/strobe-test. Update notebook index/evidence with actual outcomes.

Test atomic invalid multi-target edits, HTP ties, coherent-group activation, programmer zero, source attribution, mode/grant freeze, stale release token and unknown physical status. Preserve human holds on client loss; restart disarmed. Lux owns output safety; UI cannot claim it. Real fixture/effect capability cannot be enabled by synthetic profile.

Historical research, auditions, exhaustive matrices, long soaks, full-show renders
and physical/combined-load checks are intentionally outside the normal software
milestones unless their protected behavior changes. Retain their owning documented
on-demand commands; no private media download or test hardware side effect.
Independent builds retain lockfiles and existing repository editions; this plan
does not upgrade dependencies/editions or replace existing intra-repository workspace
paths. The ban is on new sibling-repository path dependencies.

## Resources, review and recovery of work

Local lane B on rpi5 NVMe/2 GiB class, separate checkout from GigPies; first LX-01 estimate <768 MiB compiler RSS and ≤512 MiB target growth. One shared build slot with GP-01, jobs=1. Source reasoning/fixture review can run concurrently with other lane compilation; no full-song replay or new media.

Independent fallback: LX-01 additional patch rejection vectors and documentation; outline LX-02 tests without implementing before review. No unbounded render or research assignment.
Before builds check free space and target size; below 20 GiB free or above 5 GiB
output is a review, not permission to delete another task's cache. No reduced
coverage/debug information to make a budget appear to pass.

Handoff: exact changed files, commit plus patch hashes or bounded source manifest
if uncommitted, contract IDs/versions and provider-fixture hashes, commands/results,
intentional skipped classes, remaining limits and next task/owner. Stage only named
owned changes if a later implementation session commits; no public push is implied.
Receiving owner reviews independently and writes an immutable private-ledger
acknowledgement. Interrupted work stays visible with last completed acceptance
criterion; never reset/stash/clean another session or replay an uncertain mutation.


## Implementation launch prompt

Host/cwd assignments and source preparation are in GigPies PARALLEL_WORK_PLAN.md.
This is a prompt for a later user-started session; no implementation worker has
been started by the planning pass.

```text
Work only in the current shr-lux checkout. Read AGENTS.md (if present), the
owning docs and docs/notes/0027-gigpies-implementation.md, then the referenced GP-2026-10-04.1 contracts.
Implement only LX-01; keep progress and evidence in this plan. Check hostname,
HEAD/source manifest, live Git state, active ownership and current contract hashes
before edits; preserve other sessions and unrelated work. If this is a delivered
snapshot, verify its handoff manifest and separate receiving acknowledgement first.
Reserve this host's one build slot as described in GigPies PARALLEL_WORK_PLAN.md;
use Rust 1.97.1, Cargo.lock, CARGO_INCREMENTAL=0 and cargo -j 1 in normal target/.
Run focused checks during work and the required normal suite for changed behavior.
No sibling writes, sibling path dependencies or unilateral contract changes.
No audio/MIDI/DMX/playback/device/display/service changes or shared load tests.
When blocked report exact provider/task/version mismatch and continue only the
independent fallback LX-01 additional patch rejection vectors and documentation; outline LX-02 tests without implementing before review within this repository; do not fake acceptance.
Stop after the scoped task and reviewable handoff, before later milestones,
physical operations, publication or deployment. Do not stage unrelated files or
claim mock, planned or incomplete behavior is a finished engine.
```

## Progress

- 2026-10-04: source and owner documents inspected; plan written. Implementation
  tasks remain in the states above. Physical evidence retains its original limits.

### LX-01 first software wave — 2026-10-04 (ready for review)

Source baseline `ba4ccd92656e6d2a3cbc6424cdc2a017d3e4f14d`; contract
GP-2026-10-04.1/C-LIGHT:1 unchanged. Added pure synthetic fixture/patch models,
validated stable IDs/labels, one-universe address footprint/overlap, profile
defaults/ranges and coherent RGB/pan-tilt value groups. Failed full-patch
replacement preserves the prior validated patch. Synthetic profiles are invented
and advertise no strobe, effects or real hardware map. Zoom is independent.

Added bounded typed snapshot/source vocabulary with volatile/disarmed/null-output
status, per-attribute masks, contributors/winners, inhibit/clamp and distinct
resolved/final intent/submitted/observed values. Schema validation cannot establish
authority arbitration or physical output. JSON envelope parsing/framing and timed
page assembly are LX-03; persistence and movement paths remain later milestones.

[Provider corpus](../../tests/fixtures/c-light-v1/README.md) supplies E04 prior
states, commands and expected snapshots plus actual patch acceptance/refusal
inputs. LX-01 exercises owner validation of expected snapshots, not execution of
E04 commands; LX-02 must execute and compare them independently before claiming
HTP, Hold, master or blackout acceptance. Corpus SHA-256 is retained with the data.

Measured software validation on rpi5 after lane A's explicit release:

- `cargo +1.97.1 fmt --all -- --check`: passed.
- `cargo +1.97.1 test --locked --test fixture_contract -j 1`: 9 passed.
- `cargo +1.97.1 test --locked --all-targets -j 1`: 51 passed (42 existing
  production tests plus 9 new fixture/contract regressions), none ignored.
- `cargo +1.97.1 clippy --locked --all-targets -j 1 -- -D warnings`: passed.
- `cargo +1.97.1 build --locked --release -j 1`: passed; 17.22 seconds,
  peak RSS 260496 KiB, zero swaps; normal profile retained.
- `python3 scripts/check-terminal.py`: all 8 Linux PTY recovery cases passed.
- `python3 scripts/check-simulation.py`: all 6 fast preparation contracts passed.
- `python3 tests/check-lighting-corpus.py`: JSON/TSV parity, bounds and hashes passed.

All Cargo build/check/test/Clippy/release commands held the same nonblocking
host-local parent lock with jobs=1 and incremental=0. First focused build took
4.34 seconds, peak RSS 232560 KiB, zero swaps; it exposed a TSV column indexing
error in the new test harness (8 passed/1 failed). Header-name decoding fixed
that harness before all final checks; corpus and production logic were unchanged.
Normal target grew 46523246 bytes overall (about 44 MiB); about 28.6 GiB free
remained. No cache cleanup, dependency or lockfile changes were needed.

Historical auditions, full-show/media renders, exhaustive matrices, long
benchmarks, hardware/load and live I/O were intentionally skipped. Synthetic
PTY simulation used temporary silent WAV data with no MIDI or playback flags.
This is offline model validation; standalone runtime is unchanged, and no
integrated engine, wire parser/endpoint, physical fixture or persistence is claimed.

Corpus E04 JSON SHA-256:
`c15a663df427bb52997f7a2a54687677d0dd63941eefe53ba24c24285de99256`;
patch JSON SHA-256:
`92ee854181490f337f5232fef314c11025394a514ab2bcac626bb78e275cdeed`.
Other exact corpus hashes are in `tests/fixtures/c-light-v1/SHA256SUMS`.
Next owner: coordinator and Lightdesk review LX-01 source/corpus, then separately
authorize LX-02; LX-02 is not started by this work. The task table above records
its original planning state; this artifact is ready for review, not yet accepted.

### LX-01 corrective checkpoint — task 0008

Implemented, pending coordinator review and build slot: snapshots refuse applied
AUTO until a validated active AUTO vocabulary exists; ASSIST proposal remains
separate. Every active playing, programmer and Hold source must be reported,
including sources hidden by manual priority. Fixture default is reported only
when no applied source exists. Intensity winner flags follow programmer > Hold
> playback HTP, including every tie. Non-intensity activation order remains an
engine responsibility. Stored values now explicitly distinguish Cue/Palette,
with independent global 32-ID limits (identical text IDs may exist in both).
Concrete positive/negative regressions cover omission, ties, masked sources,
unbacked AUTO, proposal separation and independent capacities. E04 corpus bytes
and normative meanings remain unchanged. No hardware/output change.

### LX-02 static authority — task 0008 (implemented, offline validated)

Coordinator accepted the LX-01 corrective checkpoint and then reviewed LX-02
source. The real owner engine now implements atomic programmer/Hold transactions,
separate copied cue/palette stores and copied playbacks, HTP with every tied
contributor, coherent non-intensity engine activation order, intentional zero,
clear-to-Hold, bounded global/per-fixture masters, blackout and Manual/Assist
continuity. Initial stores are construction-only; active seeding is refused,
preserving state/revision-bound preview validity. Record copies programmer masks
only; update leaves running copies unchanged until GO. Level zero retains
non-intensity until Off. Replacement patches must advance patch revision and
validate all retained masks. No surface simulator is imported.

[Static authority API and limits](0028-static-authority.md) documents manual use
without analysis/MIDI. Engine-created E05 intensity previews bind show, epoch,
patch, revision and scope/current/destination. Transition/validity are declared
500/2000 ms; timing/expiry and `release_commit` remain unavailable until LX-04.
Null sink inspection is intent only: volatile, disarmed, submitted/observed null,
physical state unknown. LX-03 service/wire and LX-04 timing/persistence were not
started. AUTO remains unavailable pending separately reviewed LX-05 vocabulary.

Measured validation on rpi5, after A's pass1 explicit build release:

- Formatting check passed.
- Focused `cargo +1.97.1 test --locked -j1 --test fixture_contract --test authority`:
  19 passed (11 schema/fixture and 8 authority regressions), no failures/ignored.
- Complete `cargo +1.97.1 test --locked -j1 --all-targets`: 61 passed, no failures
  or ignored (42 existing production tests plus 19 focused tests).
- `cargo +1.97.1 clippy --locked -j1 --all-targets -- -D warnings`: passed.
- `cargo +1.97.1 build --locked -j1 --release`: passed; 20.22 seconds,
  peak RSS 292880 KiB, zero swaps, unchanged release/debug profiles.
- Post-release `python3 scripts/check-terminal.py`: 8 PTY cases passed.
- `python3 scripts/check-simulation.py`: 6 preparation contracts passed.
- `python3 tests/check-lighting-corpus.py`: parity/bounds/provenance/hashes passed.

E04 authority tests execute actual owner GO/touch/clear/master/blackout commands
from each named prior state and compare logical outcomes, masks, contributors,
winners and retained color/position. Additional tests cover invalid transaction
atomicity, ties/removal, coherent position/RGB activation, copied updates,
palette/record masks, Hold/mode continuity, capacities, rounding, preview
invalidation and active-seed refusal. The stored duplicate regression now remains
within capacity and specifically requires Duplicate. Normative E04 corpus bytes
and provenance hashes are unchanged.

Initial focused compilation refused an iterator closure lifetime (E0373), before
any test execution. Explicitly capturing the Copy store kind and borrowed key
fixed compilation without changing reviewed behavior. Final focused/full/Clippy
checks passed together; the release build started only after those children
finished. Every build-producing command held the nonblocking parent host lock,
CARGO_BUILD_JOBS=1 and CARGO_INCREMENTAL=0, pinned toolchain/normal target.
The successful retry batch took 6.58 seconds focused (262688 KiB RSS), 5.95 seconds
full (244960 KiB), 2.38 seconds Clippy (197904 KiB), all zero swaps.

Normal target is retained at 812287058 bytes (~775 MiB), about 34.9 MiB above the
pre-pass 775735034-byte target. About 28.4 GiB disk space remained after builds;
no disk threshold crossed, cache deletion, dependency or lockfile change.
Private logs/manifests retain concise exact results; disposable preparation
helpers are removed after handoff. Historical auditions/research, exhaustive
matrices, full-song renders, long benchmarks and physical/load/live I/O were
intentionally skipped. PTY tests used synthetic silent data without playback or
MIDI flags. Final source and evidence are ready for coordinator review; next
owner is coordinator to accept the exact manifest and launch LX-03, then the
separately reviewed LX-04 stages, without user milestone prompts.

### LX03 wire/service — task0008 pass2/B (implemented; review pending)

Strict owner encoded C-LIGHT:1 service over real LX02 authority, bounded scoped
leases/replay/retired writer history, complete inventory and coherent chunk pages,
atomic page-budget refusal, explicit synthetic private same-UID Unix binding.
[Wire owner schema](0029-lighting-wire.md) and tests/fixtures/lx03/v1 supply exact
executable provider corpus. Eight focused production wire/IPC tests and69 total
normal tests passed, all-target warning-denied Clippy passed. Release/PTY evidence
completes in coordinator checkpoint. Historical/exhaustive/physical/live/load
classes skipped. Pinned serde/JSON/libc only; prior lock packages retained.
LX04 timing and persistence remain dependent on coordinator acceptance of LX03.

### LX04A timing — task0008 pass2/B (implemented; validation/review pending)

Coordinator accepted exact LX03-R2,72normal/Clippy/release/8PTY. Its corpus is
frozen and preserved. Injected10ms authority ticks,500ms intensity-only release
from real current/sourcecomputed destination,2000ms private previews bound to
identity/lease/epoch/show/patch/revision/targets, explicit release provenance,
manual scoped cancellation from actualcurrent and conservative statechange
freeze. Losing lease preserves Hold or lets an applied release finish.
New owner wire schema lx04-v1 and explicit --timed synthetic CLI are separate
from staticLX03. Six focused timing/regression/corpus tests prepared; Cargo
waits A's fullGP03 release. Durable LX04B remains gated on LX04A review.
[Timing and recovery design](0030-release-recovery.md).

### LX04B recovery — task0008 pass2/B (implemented; review pending)

LX04A source accepted and25focused/78normal passed; actual newtiming fixture data
separate from acceptedLX03. Private durable checkpoint/epoch registry, exclusive
lifetime owner lock, strict validated portable owner state, own0600 scratch/file
fsync/atomic rename/dirfsync withhonest errors. Every restart reserves fresh epoch
beforelistening. Human currentlook freezes intoHold, stores/master/blackout retained,
no programmer/activeplayback/release/lease/cache restored. Explicit scoped wire
checkpoint and --durable synthetic CLI, no implicit scene recall/outputarm.
Three pure recovery andsix timing checks passed inearlyfocused validation;
realowner/controlledCLI two-consecutivecrash regressions prepared. Finalnormal/
Clippy/release/PTY pendingnextassignedturn. Original drafts preserved.

### LX04B pass3 final offline validation — task0008

Implemented/offline-validated, pending root final artifact review. Verified live
rpi5 HEADba4ccd92656e6d2a3cbc6424cdc2a017d3e4f14d, all36exact pass2/B
manifest hashes and unchanged GP-2026-10-04.1 contractSHA54a887f4a300d049d3ba67077a2e0446f7a92c57c41cef9d3b7da13735a092ac.
Preserved every previous uncommitted source/planning change; no source commit/publication.

Found real boundedIO defect: read-only FIFO checkpoint open could block before
metadata validation. Added O_NONBLOCK plus privateFIFO refusal regression, maxu64
epoch exhaustion/filepreservation regression and actualService durablecommand
corpus with fullinventories/encodedpages/requests/replies. Rootaccepted corrective
R1 source and89normaltests. Actual data includes E05 zeroHold->700playback, distinct
release source,350midpointcheckpoint, errorpages, immutablecachedcommit/checkpoint
withoutrollback/falseerrorclear, recoveredepoch, oldepoch/lease/preview refusal.
Accepted LX03 corpus bytes remain unchanged. Data is frozen separately forLD04;
root alone accepts/transfers. Exactschema includes current/start/target convention.

Validation: initial23focused recovery/timing/wire/localservice passed; expanded
44lib+10recovery passed; fullnormal alltargets89passed0failed0ignored. Fmtpassed,
Clippyalltargets-Dwarnings passed after removing priorneedlessborrow, release48.70s
normalprofile passed; postrelease8LinuxPTYpassed. RealprivateCLI harness passed
E05 sampledinterpolation/endpoint/immutablecommit thenexplicitcheckpoint/two
crashedrestarts epochs1,2,3 withpreserved333Hold/master500/blackout, noGO/fade/grant.
Deterministic exact350midpoint belongs toRusttests; CLI polling claims sampling.
[Privateintegration guide](0031-local-provider-integration.md) and reusable
`python3 scripts/check-lux-service.py --binary target/release/lux-service` describe
real provider operation. Evidence/binarySHA/source-manifest in privatepass3/B.

All Cargo commands held parentnonblockingbuildflock jobs1/incremental0 pinned1.97.1
lockednormalprofiles. Fresh root14:47buildturn; released promptly afterrelease.
No sourcework/reviewwait held buildlock. Free27GiB; normalLux target1.2GiB retained.
No unknown/sharedcache deletion. Temporary synthetic sockets/children/state cleaned;
concise reproducibleevidence retained. Historical/exhaustive/audition/live/hardware/
load skipped. Remoteproductionauth, physicalfixture/DMX and audioanalysis remain
deferred. Nextowner root finalreview/LD04 actualconsumer integration; concrete
provider findings can be repaired withinthisauthorizedsoftware boundary.

### LX05 task0009 continuation — in progress

LX01–04 are accepted task0008 software baseline (89 normal Rust,8PTY historically),
not work to repeat. LX05 now consumes hash-verified root-accepted GP04-local:1
protocol and exact synthetic E09 data; provider release promotion remains a
separate gate. Independent client/calibration implementation is underway.
[Protocol and current limits](0032-analysis-subscription.md). Opt-in lx05-v1
status/grant/AUTO continuity design is submitted privately for independent/root
review. No central contract, old evidence, physical endpoint or source publication
has been changed by this worker. Fresh validation counts will replace no old evidence.

### LX05 corrective review checkpoint — task0009

Accepted GP04 release hash24a7075ecdd8bccfc202193039b3146320539df867e81947640daf56ddeb344e
was independently verified before the actual two-process check. That initial
release check passed fresh calibration, positive feature energy, explicitly granted
AUTO, retained look/provenance after provider child loss, and manual touch900
continuing. No physical endpoint/output was involved. Initial normal suite97,
Clippy, release,8PTY,6fast preparation and oldE04 corpus checks passed.
Independent review then required post-DSP/encoding source-age checks, cancellable
nonblocking connect, and candidate snapshot-budget safety. All three targeted
clock/budget/output checks now pass, including a deterministic actual15-page
candidate refusal retaining exact automatic contribution99 and a readable<=16-page
snapshot with no active grant. Saturated private-listener shutdown also passed.
Final corrected-source full suite, release and process repeat remain pending the
shared build turn; source is not yet accepted/published. [Exact current protocol](../../tests/fixtures/lx05/v1/WIRE.md)
and qualified active/loss/manual `features.json` accompany private frozen hashes
for independent/root review. Prior evidence is preserved.

### LX05 final worker validation — task0009, 2026-10-04

The preserved pass4 source manifest verified all113 files before edits. The final
ASSIST/no-grant expiry correction synchronizes engine proposal unconditionally
after final acquisition-clock expiry, so encoded snapshot and analysis status
both clear stale proposals in the same owner tick. Its deterministic clock seam
passed with the other three clock/budget/output regressions. Formatting also
applied the existing provenance Clippy correction; accepted LX05 wire/data bytes
were retained unchanged.

Final matching-source validation on rpi5:

- Focused clock/budget/output regressions:4passed.
- Complete normal `cargo +1.97.1 test --locked -j1 --all-targets`:103passed,
  zero failures/ignored (the prior final pass4 log totaled102, before this regression).
- `cargo +1.97.1 fmt --all -- --check` and all-target warning-denied Clippy:passed.
- `cargo +1.97.1 build --locked -j1 --release --bins`:passed,52.76seconds.
- Linux terminal recovery:8PTY passed; fast simulation preparation:6passed;
  E04 JSON/TSV parity, bounds, provenance and hashes:passed.
- Actual accepted GP04 release → matching Lux release: fresh explicit calibration,
  positive energy, scoped AUTO, loss freeze and manual override passed.
- Actual Lux release: E05 fade, immutable retry, checkpoint and two disarmed
  crash/restarts passed, epochs1/2/3.

Every Cargo invocation used the parent-held nonblocking task0009 helper with
incremental0/jobs1, locked dependencies and normal profiles. Initially occupied
turns exited75 without waiting on the lock; independent documentation continued.
No cache cleanup or source publication occurred. Target remains about1.3GiB;
free space about23GiB. Exact source, binary and private evidence hashes are in
the frozen worker handoff. Root owns independent runtime acceptance and promotion.
Historical media/auditions/exhaustive matrices/long benchmarks/hardware/load tests
were intentionally skipped; none protects the corrected expiry path. No physical
DMX, live capture, MIDI, playback, operator window, remote production control or
hardware safety acceptance is claimed. Old evidence is preserved.

### Final provider acceptance — 2026-10-04

Independent review and coordinator artifact verification accepted the corrected
source, matching release and actual-process evidence above. The accepted service
SHA-256 is `53b56b3b649da46c02206ea6559de851e1071ce4d81f8f2488d5a64f6b0a8b76`.
The exact consumer corpus remains unchanged. The [central milestone](https://github.com/PaolaShultz/gigpies/blob/main/docs/MODULE_IMPLEMENTATION_MAP.md#execution-checkpoint--2026-10-04)
tracks the subsequent console/module demonstration and publication; preceding
worker checkpoints retain their original validation and pending-state history.

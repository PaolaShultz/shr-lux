# Real private Lux provider integration

Implemented/offline-validated, task0008 pass3/B. This is a real null-output lighting
Authority behind local framed Service commands, with synthetic fixture-11 and show
11111111-1111-4111-8111-111111111111. No DMX/audio/MIDI/display/device operation.
Physical state remains unknown, output disarmed. [Exact schema/data](../../tests/fixtures/lx04/v1/WIRE.md)
and [timing/recovery behavior](0030-release-recovery.md) define the provider boundary.

## Private release executable

Use the owning `target/release/lux-service`, built using pinned1.97.1, locked
Cargo dependencies, normal release profile, parent host build lock, jobs1 and
incremental0. Executable SHA256 and all uncommitted source hashes are in private
task0008 pass3/B checkpoint-LX04B.json/source-manifest.json, along with release
and CLI evidence. The executable is deliberately absent from Git. Root transfers
only reviewed exact artifacts to the consumer; local paths are not deployment.

```sh
# An operator-created, unused private directory; resolve its canonical path first.
mkdir -m 700 /absolute/owned/path/lux-state
/home/shome/p/shr-lux/target/release/lux-service \
  --synthetic-private-dir /absolute/owned/path/lux-state --durable
```

Directory must be absolute/canonical, owned by effectiveUID, mode0700. Socket is
`lux-state/lux.sock`,0600; owner state has fixed0600 paths lighting-owner.lock,
lighting-epoch.json and lighting-checkpoint.json. Every CLI mode reserves and
holds a durable never-reused epoch BEFORE listening; one exclusive process owner.
`--durable` selects lx04-durable-v1, timed release and explicit checkpoint/recovery.
`--timed` selects lx04-v1 with volatile scenes. With neither flag, static LX03
is selected, with volatile scenes. Both volatile modes still reserve epochs and validate existing
owner files; they do not recall a stored scene. No arbitrary client path exists.
Counter exhaustion/corrupt/wrongshow/future/unowned/nonregular files fail closed.

## Fresh epoch and request sequence

Unix stream frames are4byte big-endian length followed by UTF-8 JSON<=65536.
To discover a fresh epoch without trusting an old session, send a read-only
snapshot envelope for the known show/module with epoch:"0" and null writer,
lease, request_id, expected_revision, body:{}. Owner epochs start at1, so the
structured reason:epoch refusal includes the current authoritative epoch. No
grant or scene action occurs. Request snapshot again using that epoch, assemble
all pages and validate identity/patch/schema before exposing state or issuing grant.

Use fresh writer identity and grant request_id:"1", expected_revision from that
snapshot, lease:null, body:{scope:"lighting-control"}. Use the returned body.lease
for strictly increasing command IDs starting2. Renew with the same scoped envelope
at500ms, before2000ms expiry. Connections are bounded30s/1024requests and3s idle;
reconnect proactively and request a fresh snapshot. A live same session may use an
exact retry after reconnect; an expired/retired writer needs a distinct identity.
Do not reissue a command with changed expected_revision under an old request ID.

A fresh logical command uses current revision. Time may advance a fade between
observation and command: a stale_revision conflict requires a new snapshot and a
new request ID. Cached exact retries retain original reply/revision/effective_tick,
and never roll the current view backward. Epoch mismatch discards all pending
writes, preview tokens, old grants and retries. Restart never implies GO or recall.

## E05 and restart reproduction

```sh
python3 scripts/check-lux-service.py --binary target/release/lux-service \
  --evidence /absolute/private/run/cli-evidence.json
```

This bounded owner harness starts only its own release executable in a0700 temporary
directory with synthetic data. Actual commands touch700, record cue, GO, touch0,
clear_to_hold, preview and commit. It polls full encoded snapshots through the
500ms fade, proves Hold absent/distinct release provenance, intermediate values and
endpoint700. It retries the original commit after completion, verifying immutable
reply without inventory rollback. The deterministic injected-time Rust corpus
additionally asserts exact350 midpoint and exact500ms endpoint; OS scheduling in
the CLI harness is sampled, not an exact midpoint timing claim.

The same real executable receives Hold333, master500, blackouttrue and explicit
checkpoint. Harness kills/waits its child, removes only its known stale created
socket, restarts twice with no additional checkpoint and verifies consecutive
fresh epochs, preserved Hold/master/blackout, stored cue but no playback/grant/
preview/fade/GO, null/disarmed output and old-epoch command refusal. All children
and disposable temporary paths are cleaned even on harness failure. Evidence
contains actual frames, sampled inventories and executable checksum.

For an operator-forced restart, stop/wait the owner process first. A crash may
leave lux.sock. Verify no live owner, then explicitly remove only that known stale
owned socket before restarting with the SAME state directory. Never remove epoch
registry or reset it to1. The CLI refuses preexisting endpoints, never automatically
unlinks them. SIGINT/SIGTERM orderly shutdown joins workers and removes its own
matching socket. A second simultaneous owner refuses under lifetime flock.

## Validation and remaining limits

Pass3 full normal89passed,0failed/ignored; Clippy alltargets warningsdenied passed;
format passed; normal release built48.70s. Actual checkpointed/error pages roundtrip
through the test-only PageAssembler. Regressions cover epoch exhaustion, all-mode
three-start/two-crash uniqueness, immutable cached checkpoint success after newer
error, oldlease/preview rejection, before/afterrename uncertainty, strict schemas,
privateFIFO/symlink refusal and exclusiveowner. Source-only R1 and data acceptance
are independent coordinator gates; final artifact acceptance belongs to root.

Historical/exhaustive/audition/media/load/live/physical suites intentionally skipped.
This private sameUID synthetic transport is not production remote authentication,
a physical fixture map, DMX output or combined-load acceptance. Provider page checker
is a test seam; consumers own their strict trusted decoder. Persistence keeps
logical human intent disarmed, not proof of electrical output.

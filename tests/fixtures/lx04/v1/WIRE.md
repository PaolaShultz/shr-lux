# LX04 v1 provider data

These are actual Service/Authority command executions. Accepted LX03 bytes remain
unchanged. `timing.json` uses lx04-v1; `durable-commands.json` and `recovery.json`
use lx04-durable-v1. Full inventories and encoded snapshot pages belong to each
command's prior/after observations in durable-commands.json. Recovery.json retains
the separate actual engine mid-release checkpoint/freeze regression.

Both schemas retain C-LIGHT:1 envelope and LX03 patch, capability, snapshot,
authority_inventory, limits, groups, master, blackout, output and physical fields.
Counters are canonical decimal u64 strings; stable fixture/store/playback/writer IDs
are owner-validated ASCII identifiers, never UI positions. Lease identity is the
(show_id, epoch, writer, issued lease) tuple, scope lighting-control. Fresh epoch
requires a fresh complete snapshot and grant1; no old command queue is replayed.
`sequence` counts observations/accepted identity parsing independently of engine
revision. Cached reply revision/effective_tick describe its original outcome.
Reject stale observation generations; never roll back a newer client inventory.

Timed additions:

- `wire_schema:"lx04-v1"` explicitly selects timed capability.
- `release:{transition_ms:500,preview_validity_ms:2000,transition,preview_available}`.
- Transition is null or `{start_tick,end_tick,current_tick,progress_ticks,
  duration_ticks,targets,physical}`. Tick counters are decimal strings, progress
  and duration are integer numbers; physical is unknown. Targets carry exactly
  `fixture,attribute,current,start,target`: start is the reviewed source value,
  target is the engine-computed destination, current is the interpolated value.
- Snapshot attribute `release` is optional/absent in unchanged static LX03, and
  is an integer or null in timed states. Source/contributor `release` is distinct
  from `hold`. Commit removes the reviewed Hold immediately. Release current
  masks ordinary playback during interpolation; endpoint removes release, exposing
  destination. Programmer and unrelated targets retain independent provenance.
- Command `release_preview {values}` issues body.token containing private nonce,
  writer/lease and exact preview show/epoch/patch/revision/tick/current/destination.
  Values select targets; caller values never define a destination. Token is an
  opaque owner capability, not a client-created state. `release_commit {token}`
  requires the exact returned parsed token and live lease. `release_cancel` clears
  unused preview. Preview expires strictly at issue+200 ticks; renew does not extend
  it. Logical tick is monotonic 10ms; fades end after50ticks, no catch-up replay.

Durable additions:

- `wire_schema:"lx04-durable-v1"`, same timed commands and capability.
- Snapshot durability is `volatile`, `checkpointed` or `error`. Static LX03 permits
  volatile only. Applied mutation is not evidence of persistence or physical output.
- `checkpoint:{available:true,recovery:"current_intended_look_frozen_into_hold",
  active_transients_resumed:false}`.
- Command `checkpoint` has no path field, same scoped writer/replay/revision rules.
  Successful body is `{application:"checkpointed",durability:"checkpointed",
  physical:"unknown"}`. Failure reason is durability; logical state/revision remain
  unchanged, diagnostic durability becomes error. An exact cached success remains
  immutable and cannot erase a newer error. Before-rename failures leave prior file;
  after-rename sync failures report replacement uncertainty.
- Private checkpoint has format shr-lux-checkpoint/version1, show_id, epoch,
  revision, patch, stores[{kind,id,values}], manual_hold, intended_look, master,
  fixture_masters[{fixture,level}], blackout. It is strict bounded JSON<=1MiB/depth12.
  Intended look is pre-master current state. Stores are copied Cue/Palette masks.
  Recovered current intent becomes Hold with revision0, manual mode, no GO,
  programmer, playback, fade, grants, preview or response cache. Master/blackout
  and stores survive. Output remains null_disarmed and physical unknown.

Page body fields: page (zero-based number), page_count (1..16),
encoding json_utf8_chunks, chunk UTF-8 string. Every page is <=65536 encoded bytes;
concatenate once in page order, then strict bounded parse. Reject mixed
show/epoch/revision/sequence/count, duplicates, missing pages, >2s assembly,
inner snapshot identity mismatch, patch mismatch and schema-invalid durability.
Inventory capacity is an encoded16page transaction budget; overbudget changes are
refused atomically, with no dropped store/provenance data.

Nonce is real OS-random128bit hexadecimal. Rust reproduction normalizes nonce only
for comparisons; committed data retains actual returned tokens and matching requests.
Synthetic output has never opened hardware or transmitted DMX.

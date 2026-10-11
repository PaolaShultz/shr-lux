# Authored musical scene director

This is an opt-in, device-free production owner path. Start `lux-service` with
`--scene-director --musical-analysis --analysis ABS_PRIVATE_SOCKET` and the
existing private directory/configuration arguments. It selects `lx06-scenes-v1`
or configured `lx06-scenes-v2`. Legacy startup and intensity grants keep their
existing schema. No hardware output, clock alignment, beat/downbeat inference,
key-to-colour mapping or mode-to-emotion rule is introduced. Acoustic usefulness,
Raspberry Pi deadlines and physical fixtures remain separate qualification.

The sole shared integration card is [GP-SCENES](../../../gigpies/docs/archive/records/2026-10-11-gp-scenes.md).
The contributor plan routes to that card. Musical evidence is described in
[0034](0034-musical-analysis.md); the existing owner authority and private store
remain responsible for manual priority and recovery.

`inventory.scenes` includes authored `map`, raw `evidence`, `interpretation`,
selected `current`, proposed ID, `reason`, elapsed-time `transition`, intended
logical values, applied variation, nullable scoped grant and the scene automatic
layer. Fixture snapshot winners and resolved values remain the actual authority
readback. A selected scene in Manual/Assist does not imply applied RGB or intensity.
Programmer, reviewed release and Hold precede the automatic layer; master and
blackout retain their existing intensity semantics. No output observation is invented.

The map is version 1, with a decimal-string revision, at most 16 roles and 32
scenes. IDs use existing bounded domain IDs. Roles bind explicit advertised fixtures,
without duplicate assignments. Each scene has ID, label, one of `intimate`,
`floating`, `driving`, `building`, `open_lift`, `release`, independent mood and
palette labels, lift and explicit role RGB/intensity appearances. The mood/palette
labels name authored choices; they do not infer colour from musical key or mode.
All colour, intensity and lift values use logical tenth-percent 0..1000.

Default appearances are authored role looks; mood and palette labels are metadata.
The RGB triplet below is the actual palette. Confidence currently gates calibrated
source reliability, rather than section-label probability. All defaults share density
0..1000, minimum confidence 600, fade 1200 ms, dwell 2500 ms, confirmation 600 ms,
variation 20 and repetition penalty 80.

| Family | RGB (0..1000) | Lift | Energy | Trend |
|---|---|---:|---|---|
| Intimate | 1000,380,170 | 300 | 0..210 | -1000..150 |
| Floating | 230,410,1000 | 450 | 160..430 | -120..120 |
| Driving | 850,220,90 | 650 | 400..1000 | -120..120 |
| Building | 630,230,780 | 600 | 220..800 | 90..1000 |
| Open/Lift | 1000,720,380 | 900 | 750..1000 | 150..1000 |
| Release | 200,460,760 | 400 | 180..900 | -1000..-90 |

Conditions include inclusive energy/density ranges 0..1000, trend -1000..1000
and minimum confidence 0..1000. Compatible predecessor IDs are explicit and
validated; an empty list permits any predecessor. Fade is 100..10000 ms,
minimum dwell 500..60000 ms, confirmation 100..10000 ms. Variation is 0..100
logical colour units, and repetition penalty 0..1000 scoring units. Unknown fields,
dangling IDs, duplicates, control characters and out-of-range values are rejected.
Fixture capability resolution supplies intensity and coherent RGB only where
advertised. Dimmer roles retain intensity; preview lists unavailable RGB per fixture.
Movement and strobe are explicitly unsupported. A scene may omit roles; prior
scene contributions on omitted targets retain their current values and become
`scene-held` with reason `scene_unmapped`. No implicit blackout/default is guessed.
A narrowed grant likewise freezes previously scoped values before taking new scope.

The director samples up to 32 energy history entries at 150 ms cadence, expiring
entries beyond 4.5 seconds; kick density is normalized by eight hits/second.
Energy is the calibrated base analyzer energy, clamped to 0..1 and scaled by 1000.
Trend is the logical-energy change across that retained window, rather than
a per-second rate. It interprets sustained rise/drop, scores eligible scenes by range fit, continuity,
recurrence and bounded eight-selection repetition history, and breaks ties by ID.
A repetition penalty alone cannot change a scene under stable evidence. Candidate
confirmation and minimum dwell apply; a normal transition completes before another
normal switch. Uncertainty cancels pending/fade intent to the actual current values;
source loss/generation change resets runtime history and section associations.

Up to sixteen local arrangement identities retain stable scene associations,
evicting the oldest learned association when a new identity exceeds that bound.
Identity admission depends on low descriptor novelty or supported recurrence,
independent of tentative verse/chorus-label confidence. Building, release and arrival
families do not establish persistent section associations. A recurring identity gets
its stable scene preference when eligible; conditions/compatibility remain binding.
Variation offsets supported red upward and blue downward within authored bounds;
recurring identity uses the base authored look. Related/stationary harmonic evidence
keeps the authored fade; distant `other` movement extends it by one quarter, capped
at ten seconds. Harmony alone never selects a different scene. Timing is elapsed
owner time, with no guessed bar or beat boundaries.

Commands use the existing C-LIGHT:1 leased `kind:command` envelope, envelope
`expected_revision`, and body `{scope:"lighting-control",command:{action:...}}`:

- `scene_map_replace` contains full `map`, whose revision must be current plus one.
  Validation, snapshot admission and crash-safe private store replacement precede
  the atomic visible change. Source automation is revoked after a successful edit.
- `scene_preview` contains `scene_id`. Its `applied` body has application
  `scene_preview`, scene ID, map revision, target `values`, compact actual target rows `{fixture,attribute,value,source}`,
  per-fixture unsupported attribute lists and physical `unknown`. It changes no
  authority revision, map, selection or grant.
- `scene_grant` contains explicit `fixtures`, `intensity_cap`, `color_cap` and
  `ttl_ms`. Caps are 0..1000; TTL is 10..2000 ms in ten-ms units. It requires
  fresh calibrated evidence, AUTO, matching source generation, patch and writer
  lease. Its compact ACK contains application `scene_granted`, map revision and
  the exact grant object; full state is read through paginated inventory. Every
  owner poll rechecks these boundaries. It replaces the prior analysis
  grant through the same authority. A subsequent legacy intensity grant replaces
  scene intensity in its scope while retained colour remains explicitly held.

Authored maps persist as private `lighting-scenes.json` using the existing owner
store and atomic/fsync/rename conventions. Restart with the opt-in restores the
map, rejects malformed/wrong-show maps, and restores no grants, director selection,
source identities, history or active fades. Existing checkpoint recovery remains
Manual, disarmed and frozen into Hold. Authored map persistence does not checkpoint
volatile selected intent or grant state.

Software checks include model/schema, deterministic confirmation/dwell/recurrence,
harmonic-only stability, caps, manual/Hold and loss, actual leased edits/preview,
map recovery and the production PCM-analysis/authority/service producer corpus at
`tests/fixtures/scenes/producer.json`. Reproduce the small corpus with
`GP_SCENES_FIXTURE_OUT=tests/fixtures/scenes cargo test --locked --lib scene_producer_authority_loss_and_real_command_preview`.
Normal validation commands and results are retained under ignored `local/gp-scenes`.
Historical auditions, media generation, exhaustive research, physical output and
load experiments are intentionally excluded from this software task.

# Configured synthetic authority and named analysis

Implemented software integration, with physical output still null/disarmed and
unknown. [LX05 baseline](0032-analysis-subscription.md) and
[private restart](0030-release-recovery.md) retain their legacy semantics.

`lux-service --synthetic-private-dir ABS_PRIVATE_DIRECTORY --config ABS_JSON`
loads an explicit startup identity. The version1 JSON has `show_id` (canonical
UUID), `patch` (the existing complete `PatchSpec` with canonical decimal-string
`patch_revision`) and optional `analysis`. Only existing synthetic personalities
are admitted. Config input is a regular nonsymlink file, at most65536bytes;
duplicate keys, unknown fields, nested malformed patch data and excess depth
refuse before persistence or socket creation. The existing fixture and wire
resource limits still apply. No physical personality, discovery or output driver
is introduced.

The optional analysis binding is exactly
`{"version":2,"inputs":["input-17","input-03","input-09","input-01"]}`.
The four distinct canonical logical IDs identify the ordered semantic sources
kick, bass, guitar-1 and guitar-2, independently of physical input slots. Their
unsigned16bit identity domain is not a qualified channel-count claim. A binding
requires an explicit `--analysis ABS_PRIVATE_PROVIDER_SOCKET`. Without a binding,
`--analysis` retains its legacy v1 meaning, even with a configured show/patch.
`--timed` and `--durable` retain their existing independent meanings; durable
includes timed behavior.

## Consumer contract

Configured analysis sends only
`{"subscription":"lux.aux.v2","version":2}` and validates every descriptor on
every connection against the exact ordered configured IDs. It admits only
C-ANALYSIS:2/lux.aux.v2, retaining all existing stream3/48kHz/raw-pre-fader,
canonical-counter, frame-origin, same-host acquisition-age, loss, sequence and
calibration checks. Wrong mapping, version or order never downgrades to v1.
`Descriptor::decode` and `Client::start` remain strict legacy entrypoints; the
configured entrypoints require an independently validated `Binding`.

Configured service inventory/capabilities always declare `wire_schema:lx05-v2`,
including when the source is absent. Inventory adds `analysis_binding`, and the
inner `analysis` status adds the same `analysis_binding`. A standalone
`analysis_status` reply retains body shape `{wire_schema,analysis}` with that
binding inside `analysis`. The binding has exactly `version` and `inputs`.
All previous LX05 command shapes remain unchanged. The estimator identity remains
`lux.analysis.v1`; this change adds no musical estimator or DSP.

Consumers must validate both the current source descriptor and each retained
AUTO contribution's `provenance.source_identity` against the explicit binding,
even with an absent source. Retained contributions may legitimately name an older
epoch/map than the current live descriptor; equality to current epoch/map is not
required. Reattachment invalidates calibration and grants, and missing samples
never become synthetic silence. Human control/Holds remain independent.

The owner-produced [configured corpus](../../tests/fixtures/lx05/v2/README.md)
covers cold absent configuration, active analysis, retained provenance after
source loss and manual control. The producer descriptor is copied unchanged from
GigPies configured analysis commit527e320e778155597be4668344d0b4fde7e2550b.
Actual physical acquisition timestamps are not established by this software path;
GigPies rejects configured publication with its physical provider option.

## Private startup identity and recovery

Use a fresh owned0700 canonical private directory for first configured startup.
Under the existing exclusive owner lock, Lux persists `lighting-startup.json`
with the complete normalized startup identity (show, patch and optional analysis
binding). Existing legacy storage cannot be adopted implicitly. Legacy startup
cannot open configured storage. Later configured starts must match exactly;
changes require a separately chosen fresh directory, not overwriting saved state.

The identity and any checkpoint patch are checked before consuming a new epoch or
modifying checkpoint/socket state. Invalid/mismatched files are preserved. A
missing epoch registry after previous ownership fails closed, including a crash
between initial identity and epoch writes; use fresh storage after review rather
than guessing a never-used epoch. Failed first startup may retain an owner lock
file, as existing recovery policy already does.

Durable recovery uses the configured show and patch, keeps intended values/Holds,
and resumes neither AUTO grants nor active transients. A runtime replacement patch
can still be reviewed/applied through existing commands, but checkpointing a patch
that differs from the startup identity refuses; select a reviewed new startup
configuration and fresh private directory for a different persistent patch.

## Reproduction and limits

Normal gates are the repository's pinned-toolchain default tests, warning-denied
all-target Clippy, formatting, release build, terminal recovery and fast simulation
checks. `cargo test --locked --test configured_service` covers startup admission,
identity preservation, recovery and real private socket reconnection. The normal
`configured_owner_corpus_active_absent_retained_and_manual` unit test checks exact
owner output against the accepted fixture. Explicit fixture refresh is
`LUX_GENERATE_LX05_V2_CORPUS=1 cargo test --locked --lib configured_owner_corpus_active_absent_retained_and_manual`;
review the diff before accepting any change. No automatic fixture rewrites occur.
Hardware, audible playback, MIDI/DMX output and deadline qualification remain
separate; no service starts automatically on boot.

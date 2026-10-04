# C-LIGHT:1 E04 provider corpus

Decision: GP-2026-10-04.1. Provider baseline:
`shr-lux ba4ccd92656e6d2a3cbc6424cdc2a017d3e4f14d` plus the uncommitted
LX-01 source manifest in lane B's handoff. Contract SHA-256:
`54a887f4a300d049d3ba67077a2e0446f7a92c57c41cef9d3b7da13735a092ac`.

`e04.tsv` is UTF-8 tab-separated semantic data, not a JSON wire message. Every
row names prior state, command, playback inputs, programmer/Hold masks, master,
blackout and expected intensity/source/winners; fixed RGB/position expectations
prove that blackout retains those attributes. IDs are `fixture-11`,
`playback-1`, `playback-2`; rig is synthetic-rgb-position-v1 at address 1,
patch revision 1, show `11111111-1111-4111-8111-111111111111`, epoch 9.
`null` is unavailable/absent; zero remains an authored value. Recordable
programmer is exactly the programmer column: clear-to-Hold removes its mask.

LX-01 tests instantiate and validate these expected snapshots through actual
owner schema/capability logic. They do **not** execute commands or prove HTP,
Clear-to-Hold or master/blackout arbitration. LX-02 must consume the same data,
execute the commands and compare its results before authority acceptance. Real
profiles, physical output, persistence and framed JSON remain unavailable.
Consumers copy these files byte-for-byte after review, preserving SHA256SUMS.

`patch.tsv` contains valid last-slot/adjacent footprints and invalid zero/overflow,
overlap and default inputs; provider tests build and validate complete patches.
RGB and pan/tilt are separate coherent groups; zoom is an independent angular
attribute. No shutter/strobe/effect attribute or real fixture mapping is advertised.

The Rust API is typed and pure. A future LX-03 adapter must enforce UTF-8 JSON,
unknown/duplicate fields, 64 KiB/depth limits and complete timed snapshot paging
before constructing these models; no parser, socket or pagination assembler is
implemented here. Typed snapshots have finite fixture/attribute/value/contributor
capacities, and represent complete snapshots rather than transport pages.

`e04.json` mirrors the TSV rows for consumers that decode JSON. It is a corpus
container (including provenance/rig), **not** a C-LIGHT envelope or an advertised
wire schema. `python3 tests/check-lighting-corpus.py` checks exact TSV/JSON semantic
parity, common limits and hashes. Owner Rust tests consume TSV and validate patch
and expected snapshot models; the JSON check alone is not engine acceptance.

`patch.json` is the explicit synthetic capability/patch data for the E04 rig:
units, inclusive bounds/defaults, footprint and coherent groups. It is likewise
corpus data rather than an envelope. Its defaults match `SyntheticMode::RgbPosition`.

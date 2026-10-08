# LX05 configured consumer corpus

`configured-descriptor.json` is the exact GigPies producer fixture at
527e320e778155597be4668344d0b4fde7e2550b (its configured descriptor test checks the
actual serializer). `configured.json` is produced by Lux's independent configured
decoder, existing analyzer and actual authority/service, not hand-written UI data.
Its synthetic E09 sample content is reused with matching configured source headers.
The test clocks are injected; this is not physical acquisition evidence.

`wire_schema` is `lx05-v2` in every configured inventory/status. Explicit binding
occurs at inventory `analysis_binding` and inner status `analysis.analysis_binding`.
Both equal `{version:2,inputs:[four exact ordered canonical IDs]}`. The standalone
status body is `{wire_schema,analysis}`. No extra top-level binding is added there.
The estimator remains `lux.analysis.v1` and LX05 commands are unchanged.

Corpus keys: `config`, `cold_start` (custom show/two fixtures, absent source),
`status` (request/replies), `active`, `source_absent_retained`, `manual`.
Retained AUTO provenance must match the configured binding even with no current
source. An older retained epoch/map is legitimate; no AUTO grant survives loss.
The cold-start binding must not be inferred from an absent source descriptor.

Existing lx05/v1 files remain unchanged. Reproduction is documented in
[the owning note](../../../../docs/notes/0033-configured-analysis.md).

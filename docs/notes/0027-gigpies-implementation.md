# SHR Lux: plan and implementation

Lux owns lighting authority, fixture capabilities, programmer/Hold/cue/playback,
analysis, timing, effects and physical output. Lightdesk presents that authority;
GigPies supplies named source analysis and final system integration.

## Delivered contract and evidence

LX-01..05 have software acceptance: fixture/patch contracts, static authority,
private service, timed release/durable disarmed restart and named PCM analysis with
bounded ASSIST/AUTO. Actual provider/consumer execution uses null output.
[Authority](0028-static-authority.md), [wire](0029-lighting-wire.md),
[release/recovery](0030-release-recovery.md), [provider](0031-local-provider-integration.md)
and [analysis](0032-analysis-subscription.md) own exact behavior/evidence.
Beat/downbeat/harmony and physical light are not implied by source activity.

## LX-06 — physical fixture output

State: PLANNED; physical acceptance requires a separate authorized session and known
fixture/adapter/patch facts. Owner: SHR Lux. Outcome: one bounded output worker with
explicit arm/rearm, exact fixture encoding and visible submission/fault state.
Next: establish the real profile and loss/blackout/recovery policy, then prepare
software contracts/tests before any output. Do not invent fixture maps, movement
limits or generic all-zero safety. `src/dmx.rs` currently encodes ranges, not USB writes.
Acceptance: encoding/worker bounds, unknown versus submitted versus observed output,
fault/restart/rearm and fixture-specific physical checks. No automatic rearming.
Progress/evidence: no physical output implementation or qualification.

## Later engine expansion

Cue-list/fade/effect/movement and venue patch authoring need explicit scope and one
card here when selected. Preserve manual Hold and per-attribute source explanation.
These are owner algorithms; Lightdesk must not substitute its simulator. Shared
whole-show qualification lives in GP-H3, not a duplicate Lux load tracker.

## Tracking and verification

This is the owning plan for module-only GigPies work. Keep each new task's plan,
implementation state, checklist, evidence and next action together here. Shared
integration tasks live only in the [GigPies integration plan](https://github.com/PaolaShultz/gigpies/blob/main/docs/MODULE_IMPLEMENTATION_PLAN.md);
link to their cards instead of copying status. Follow its task lifecycle and this
repository's AGENTS.md. STATUS/acceptance files hold dated evidence, not another queue.
Preserve closed milestone details in the linked archive; do not reopen old launch cards.
A documentation reconciliation does not rerun tests or qualify hardware.

[Historical plan and milestone evidence](archive/tracking-before-2026-10-09/0027-gigpies-implementation.md). Its launch instructions are retired.

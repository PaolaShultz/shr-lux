# Development milestones

Date: 2026-09-28

Status: proposed sequence. Tags: #planning

## 0 — Foundation (this scaffold)

Runnable Rust TUI shell, descriptor-only doctor, pure DMX address/range contracts,
linked notebook, working agreements, normal tests and CI. No lighting output.

## 1 — One real fixture

Record fixture model/mode/address, install the scoped USB permission rule, implement
bounded uDMX transport, explicit output arming and fixture-aware blackout. Verify one
known dimmer/color at low level, then confirm disconnect/reconnect and exit behavior.
Record observations in a dated note. See [bench checklist](0011-fixture-bench-test.md).

## 2 — Fixture abstraction and manual operation

Versioned venue patch, overlap/range validation, capability mapping, steady scenes,
master level, blackout latch. Define ownership and persistence; then add MIDI learn
and actual controller feedback once hardware is identified.

## 3 — Musical response

Capture and replay source feeds. Prove activity/transient detection, then add rhythm
and confidence-driven scheduling. Hold looks through uncertain input; enforce dwell
and transition rules to avoid flickering decisions.

## 4 — Richer understanding

Evaluate bars, section changes, harmony and performer focus on actual band material.
Measure false transitions as well as detection accuracy. Keep manual correction usable.

## Open hardware facts

Exact borrowed cans and channel maps; Arturia model; audio interface models; available
mixer outputs; cable/termination arrangement; measured USB/DMX latency and output behavior.

Related: [project map](0001-project-map.md), [original idea](../../idea.md).

[Notebook index](../index.md)

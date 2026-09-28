# Architecture and state ownership

Date: 2026-09-28

Status: initial engineering direction, not all implemented. Tags: #architecture

Use one Rust package with a library and CLI until real boundaries justify a workspace.
Current modules: `dmx` (address/range contracts), `hardware` (descriptor inspection),
`ui` (rendering); `main` owns CLI routing and terminal lifetime.

## Planned pipeline

```text
Audio devices -> source features -> musical estimates -> show decisions
MIDI / TUI -------------------------------------------> manual actions
Venue patch + show decisions + overrides -> resolved fixture state -> DMX
Resolved state ---------------------------------------> TUI + MIDI LEDs
```

Retain source identity, monotonic timestamps, confidence, and freshness in estimates.
Unavailable is distinct from silence, zero energy, or low confidence. Keep inferred
musical state separate from artistic policy and final device encoding.

Audio callbacks must not wait for USB, redraw the TUI, or write files. Introduce bounded
queues and explicit overload behavior with the actual audio backend. USB output belongs
to one worker with bounded transfers. UI receives snapshots; it does not clock the show.
Avoid committing to thread count, queue sizes, or update frequency before measurement.

Manual priority proposal: explicit blackout latch > manual ownership > automation.
Resolve per parameter, and define releases before implementation. Never timeout a
blackout latch. Fixture-aware blackout may require specific shutter/dimmer values;
an all-zero universe is not a universal definition of safe fixture state.

Use a null output sink first; arm real output explicitly once a validated patch exists.
Persist configuration atomically with a schema version when persistence is added.
Do not write transient automatic state back into user configuration on every beat.

Related: [musical analysis](0008-audio-and-musical-state.md), [MIDI](0009-midi-feedback.md),
[TUI](0007-terminal-contract.md), [roadmap](0010-roadmap.md).

[Notebook index](../index.md)

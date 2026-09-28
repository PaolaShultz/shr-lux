# MIDI control and feedback

Date: 2026-09-28

Status: device model and protocol not established. Tags: #midi #controller

User owns an Arturia controller with controllable colored pads, roughly seven or eight
colors. Exact model, firmware, MIDI ports and LED protocol are still needed. Do not
assume generic Note On velocity maps to colors, or that brightness is supported.

Input mappings should produce semantic actions. Feedback derives from resolved active
state so automatic changes, TUI actions and MIDI actions agree. Avoid MIDI feedback loops:
a sent LED update must not be interpreted as another user action.

Define whether each control is momentary, latched, absolute, or relative. Non-motorized
faders need a deliberate pickup/takeover strategy. No touch sensor is assumed.
Blackout stays latched until explicit release; an LED should distinguish it from an
unplugged or unknown controller state.

Potential Rust backend: [midir](https://github.com/Boddlnagg/midir). No MIDI dependency
or guessed device mapping is added yet. Obtain the model's official Arturia documentation
before implementing LED output.

Related: [architecture](0005-architecture.md), [roadmap](0010-roadmap.md).

[Notebook index](../index.md)

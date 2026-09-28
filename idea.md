# shr-lux

## Idea specification

**Project:** shr-lux  
**Platform:** Raspberry Pi  
**Language:** Rust  
**Interface:** TUI, targeting an 80×25 terminal  
**Purpose:** Autonomous, musically responsive DMX lighting for small live gigs, with manual MIDI control and visual feedback on the controller.  
**Status:** Concept specification, consolidated from this conversation on 2026-09-28.

This document preserves the user's project ideas and the architectural suggestions and examples discussed in the thread. Examples are illustrative, not fixed mappings or implemented capabilities. Hardware compatibility, algorithms, timing targets, and exact UI layouts have not yet been selected or tested.

## 1. Core concept

Build a compact, autonomous lighting runner that listens to separate live instrument feeds from a mixer, analyzes them in real time, and runs a lighting show appropriate to the equipment and stage layout at the venue.

The band plays normally. The system follows the performance without requiring a lighting operator, a prepared song timeline, backing tracks, or an external MIDI clock. A MIDI keyboard, pad controller, DAW controller, or other suitably mappable controller provides manual intervention when wanted.

The central workflow discussed is:

> Patch the available lights, patch the available band channels, configure the show for the venue, and press RUN. The system follows the musicians during the gig.

Existing lighting software offers many capabilities, but the intention here is a focused tool matching this particular small-gig workflow. It should do more than flash a light on every kick: it should use musical context, instrument identity, and known performer positions to make lighting decisions.

## 2. Hardware concept

### 2.1 Main unit and screen

- Raspberry Pi as the host; the exact Pi model is not specified in this thread.
- Rust implementation, following the user's preference for Raspberry Pi and TUI projects.
- Possible 7-inch or 10-inch HDMI display.
- Fixed 80-column × 25-row terminal as the intended UI size.
- Some graphics are desirable; the rendering approach remains open.

### 2.2 Four-USB-device arrangement

The user described using four USB connections:

| Connection | Device | Intended role |
|---|---|---|
| USB 1 | Audio interface 1 | Capture instrument feeds |
| USB 2 | Audio interface 2 | Capture additional instrument feeds |
| USB 3 | Existing USB-to-DMX adapter | Send lighting commands using uDMX |
| USB 4 | MIDI keyboard/controller or DAW controller | Manual control and, where supported, visual feedback |

An example audio assignment discussed was:

| Interface | Input 1 | Input 2 |
|---|---|---|
| Audio interface 1 | Kick | Bass |
| Audio interface 2 | Guitar 1 | Guitar 2 |

This assignment assumes the chosen devices actually provide two independently capturable input channels each. It is an example configuration, not a claim about an unspecified cheap USB sound card.

### 2.3 Audio hardware

- Receive two or more channels from the mixer.
- Initial example: kick and one guitar sent through mixer AUX outputs.
- Expanded example: kick, bass, guitar 1, and guitar 2 across multiple interfaces.
- AUX or suitable direct outputs were discussed as possible signal sources.
- Inexpensive class-compliant USB audio devices are candidates because the inputs are for analysis, not high-fidelity PA reproduction.
- Extreme recording quality is not a requirement.
- 16-bit capture at 44.1 or 48 kHz was mentioned as a plausible starting point, not a selected format.

The intended benefit depends on the feeds containing the assigned sources. Calling a channel “Guitar 1” does not isolate that instrument if its mixer send actually contains a mixed signal.

### 2.4 DMX hardware

- The user already owns an inexpensive USB-to-DMX adapter using the uDMX protocol.
- The user identified an open-source basis for using it.
- That existing adapter is the intended lighting output hardware.
- Exact adapter identification, transport implementation, output rate, and fixture patch remain to be established.

### 2.5 Controller hardware

- Support a MIDI keyboard/controller, a DAW-style controller, or another device that can be properly mapped.
- The user's Arturia controller has illuminated pads with approximately seven or eight different available colors, as described by the user.
- The user states that these pad lights can be controlled.
- Exact Arturia model, palette, MIDI messages, and feedback protocol were not supplied.
- Brightness control and other feedback features depend on the chosen hardware and must not be assumed from color support alone.

## 3. Audio analysis and musical understanding

### 3.1 Source-aware analysis

Analyze each configured input separately. Knowing which input represents which instrument is a central feature of the project.

| Source | Information discussed |
|---|---|
| Kick | Transients, hit strength, rhythmic timing |
| Bass | Activity, rhythm, notes/pitch |
| Guitar 1 | Activity, chords/harmony, possible solo context |
| Guitar 2 | Activity, chords/harmony, possible solo context |
| Combined musical state | BPM, beat phase, bars, energy, section changes, chord progression |

Separate feeds give the system a clearer basis for recognizing instrument activity than analyzing only the complete stereo mix.

### 3.2 Desired musical state

The conversation covered tracking or estimating:

- Tempo/BPM.
- Beat timing and phase.
- Downbeats and bar boundaries.
- Current bar position or count.
- Instrument activity and relative levels.
- Kick-hit strength and transient events.
- Overall energy/intensity.
- Notes and rhythm from bass.
- Guitar chords and chord progressions.
- Current key, as an additional proposed state/display item.
- Song-section changes.
- Possible solo activity.
- Situations where only one instrument is playing.
- Song endings or a collapse in musical energy.
- Confidence in estimated musical information.

These are desired analysis capabilities. Reliable automatic recognition of solos, downbeats, sections, and harmony is not established by this concept document.

### 3.3 Suggested development priority

The assistant proposed the following usefulness order:

1. Beat and phase.
2. Bars.
3. Energy.
4. Instrument activity.
5. Song sections.
6. Harmony and chords.

This was a proposed implementation priority, not a user decision to remove chord-progression tracking from the project. Harmony remains part of the user's original idea.

## 4. Architecture discussed

Separate audio analysis, musical interpretation, artistic behavior, and hardware output.

### 4.1 SENSE

Inputs:

- Live audio from multiple USB interfaces.
- MIDI controller actions.
- Manual controls through the TUI.

Responsibilities:

- Capture configured channels in real time.
- Associate channels with instrument identities.
- Receive controller messages and turn them into configured actions.

### 4.2 UNDERSTAND

Responsibilities:

- Analyze individual instruments.
- Combine timing and activity information into a musical state.
- Track beat, bar, energy, harmony, instrument activity, and possible section/solo changes.
- Expose estimates and confidence to the show engine and user interface.

Illustrative event vocabulary from the discussion:

```text
Beat
Downbeat
BarStart
KickHit(strength)
BassActivity(level)
GuitarActivity(level)
Chord(Am, confidence)
Energy(0.72)
SectionChange
```

These names illustrate a separation of responsibilities; they are not a finalized Rust API. Guitar events need to retain which configured guitar/source produced them.

### 4.3 PERFORM

Responsibilities:

- Interpret musical events according to the chosen theme, rules, scenes, and macros.
- Apply the venue's available fixtures and performer-position mappings.
- Combine automatic behavior with manual control.
- Translate logical fixture actions into DMX output.
- Send supported visual state feedback to controller LEDs.

The same musical analysis should be reusable with a small set of PAR fixtures or a larger setup containing washes, movers, and other lights.

## 5. Show configuration and fixture abstraction

### 5.1 Venue setup

Before a gig, configure the system according to the equipment available at that location:

- Available fixtures and their DMX patch.
- Functional fixture groups.
- Audio input assignments.
- Guitar 1 and Guitar 2 performer positions.
- Which lights illuminate each performer.
- Available scenes, effects, themes, and musical response rules.
- Controller mappings and supported feedback.

The discussion ranged from approximately four PAR cans to a larger example with twenty fixtures and movers. These illustrate adaptability; they are not a minimum or maximum capacity specification.

### 5.2 Logical fixture control

Artistic behavior should address meaningful lighting functions, with the patch translating those functions into fixture-specific DMX values.

Illustrative actions:

```text
G1_FRONT.intensity = 0.8
G1_FRONT.color = warm
BACK_WASH.energy = 0.4
MOVERS.target = guitar_1
```

Example logical groups discussed:

- Front lighting.
- Back/rear wash.
- Movers.
- Strobe.
- Guitar 1 front light.
- Guitar 2 or opposite-side light.

Position-aware lighting means the system knows which configured lights or mover targets correspond to a performer. Automatic physical position tracking was not requested.

### 5.3 Musical effect scheduling

A kick need not directly trigger a flash. Examples discussed include:

- Advance an animation by one step on a kick.
- Change movement at a bar boundary.
- Increase strobe density when combined kick and bass energy rises.
- Run beat-synchronized chases.
- Schedule transitions on the next beat, bar, or multi-bar boundary.

## 6. Instrument-aware and spatial behavior

The user explicitly wants the system to know where Guitar 1 and Guitar 2 are on stage and illuminate them when appropriate, including solos and passages where only one instrument is playing.

Illustrative rules discussed:

| Musical condition | Possible lighting response |
|---|---|
| Guitar 1 solo detected | Raise Guitar 1 front light, dim the opposite side slightly, and focus a mover toward Guitar 1 if available |
| Guitar 2 is the only active instrument | Spotlight Guitar 2 and reduce the background wash |
| Kick and bass show high energy | Strengthen beat-linked movement |
| Quiet vocals or instrumental section | Use wider stage effects |
| Song ends or energy collapses | Fade toward an idle scene |

The vocals example was an assistant suggestion. The four-input kick/bass/Guitar 1/Guitar 2 arrangement does not include a dedicated vocal feed; vocal-state detection would need an appropriate information source or a separate decision about inference.

Solo detection is an intended musical interpretation, not simply a confirmed synonym for whichever guitar is loudest. Its exact definition and method remain open.

## 7. Manual control

### 7.1 Mappable controls

Allow practical live control through supported faders, knobs, keys, buttons, and pads. The system should be adaptable to the controller the user has available.

One illustrative mapping proposed in the conversation:

| Physical control | Example function |
|---|---|
| Fader 1 | Master intensity |
| Fader 2 | Front wash |
| Fader 3 | Rear wash |
| Fader 4 | Movers |
| Knob 1 | Movement speed |
| Knob 2 | Effect intensity |
| Pad 1 | Blackout |
| Pad 2 | White scene |
| Pad 3 | Solo mode |
| Pad 4 | Manual scene |

These are examples, not fixed assignments.

### 7.2 Interaction with automation

The assistant proposed temporary, parameter-level manual overrides:

- A manual action takes control of the relevant parameter.
- Other automatic behavior can continue.
- On an appropriate release or timeout, the parameter smoothly returns to automatic control.

The takeover method, timeout, return fade, and handling of latched actions such as blackout were not defined. Physical touch/release detection cannot be assumed for every controller.

## 8. Controller LEDs as a second visual interface

### 8.1 Bidirectional operation

The controller is both an input device and an output display. A pad can select a theme while its illumination confirms which theme is active.

This is an explicit part of the user's idea: use the colored Arturia pad lights for clear, useful feedback during a performance.

### 8.2 Feedback possibilities discussed

| Visual behavior | Possible meaning |
|---|---|
| Pad color | Active lighting theme or scene family |
| Brightness, where supported | Intensity or energy |
| Blinking/pulsing | Beat or bar synchronization |
| Pad group | Current mode |
| Distinct color | Manual override, warning, blackout, or solo state |

Examples:

- Guitar 1 solo detected → illuminate the Guitar 1 pad more prominently.
- High-energy section → pulse the active theme pad.
- Manual override → change the relevant pad's color.

Feedback should reflect the system's actual active state, including changes made automatically or through the TUI.

### 8.3 Alternative eight-pad layout

Another illustrative layout discussed was:

| Pad | Function |
|---|---|
| 1–4 | Theme selection |
| 5 | Auto/manual |
| 6 | Solo-follow |
| 7 | Blackout |
| 8 | Panic/default |

This is an alternative to the earlier example mapping, not an additional simultaneous assignment. The meanings of panic/default and mode switching still need to be specified.

### 8.4 Output backend

Treat controller feedback as an output backend alongside DMX. The exact MIDI or SysEx messages must match the specific controller. The earlier discussion mentioned standard MIDI/SysEx as possible mechanisms but did not verify a device protocol.

The screen provides detailed information; illuminated controls provide fast, peripheral feedback while playing.

## 9. TUI and graphics

### 9.1 Display goals

- Work within an 80×25 terminal.
- Be readable on a 7-inch or 10-inch HDMI screen.
- Show the live musical state and current lighting behavior.
- Provide setup, patching, rules, live operation, and override access.
- Include simple graphics where useful.

Graphics suggested in the discussion:

- Unicode level meters.
- Beat indicators and beat phase.
- Small waveform or spectrum views.
- Fixture-state representations.

The choice between terminal-only graphics and another graphics approach remains open.

### 9.2 Proposed live-screen contents

| Area | Information |
|---|---|
| Header | Project name, BPM, time signature, bar count |
| Kick row | Input meter, beat/hit indicators, confidence |
| Bass row | Input meter, notes, active/inactive state |
| Guitar 1 row | Input meter, chord, chord confidence |
| Guitar 2 row | Input meter, chord or unavailable indicator, activity |
| Musical state | Section, energy, key |
| Lighting groups | Group name, current effect, level, next scheduled change |
| Navigation | Setup, Patch, Rules, Live, Override |

The original mockup used example values including 124.2 BPM, 4/4, bar 37, a VERSE label, 42% energy, and A minor. These were illustrative, not real measurements, fixed defaults, or evidence of working section recognition.

Example lighting rows from that mockup:

| Group | Effect | Next change |
|---|---|---|
| Front | Warm wash | Bar |
| Back | Slow chase | Beat |
| Movers | Left/right sweep | Four bars |
| Strobe | Disabled | None |

Proposed function-key navigation:

| Key | Page/action |
|---|---|
| F1 | Setup |
| F2 | Patch |
| F3 | Rules |
| F4 | Live |
| F5 | Override |

The final visual arrangement still needs to fit the selected terminal and font; the thread's mockup was a concept, not an implemented layout.

## 10. Intended gig workflow

1. Connect the audio interfaces, uDMX adapter, MIDI controller, and optional HDMI display to the Raspberry Pi.
2. Connect the chosen mixer sends to the audio inputs.
3. Assign each input to its instrument.
4. Patch the available lights and configure logical fixture groups.
5. Associate performer positions, especially Guitar 1 and Guitar 2, with their lights or mover targets.
6. Configure or select themes, rules, scenes, and controller mappings.
7. Start autonomous operation.
8. Let the system follow the live band while showing musical and lighting state on the TUI and supported controller LEDs.
9. Intervene through the controller or TUI as needed, including theme changes, level changes, manual scenes, and blackout.

## 11. Decisions still open

The conversation did not settle the following implementation details:

- Raspberry Pi model, operating system, and deployment arrangement.
- Exact USB audio interface models and independently available input channels.
- Mixer routing and channel availability at each gig.
- Audio capture backend, sample format, buffering, and timing across interfaces.
- Numeric latency, CPU, memory, or DMX update-rate targets.
- Beat, bar, chord, key, solo, and section-detection algorithms.
- Confidence thresholds and behavior when musical state is uncertain.
- Time-signature handling; 4/4 appeared only as a mockup example.
- Specific uDMX adapter identification and integration details.
- Fixture profiles, DMX addresses, channel modes, and supported fixture functions.
- Theme definitions, rule representation, configuration format, and persistence.
- Exact Arturia model, pad palette, LED messages, and supported brightness behavior.
- Controller mapping mechanism and whether any automatic learning UI is wanted.
- Override priorities, latching, timeout, and smooth return behavior.
- Final TUI toolkit, fonts, graphical rendering, and screen layout.

No specific Rust crates, repository structure, implementation schedule, or completed code were established in this thread.

## 12. Project identity

**shr-lux is a portable autonomous lighting engine for live bands:** a Raspberry Pi running Rust with a compact TUI, listening to known instrument feeds, interpreting musical activity, and driving the venue's DMX lights with optional manual control and colored controller feedback.

Its defining combination is source-aware audio analysis, awareness of configured performer positions, venue-specific fixture mapping, autonomous show behavior, and a bidirectional MIDI control surface.

# Audio inputs and musical estimates

Date: 2026-09-28

Status: requirements and research questions. Tags: #audio #analysis

Preserve separate source identity: kick, bass, Guitar 1, Guitar 2. Mixer routing must
actually isolate the intended source. AUX output names alone do not establish that.
Confirm whether each cheap interface has two independent inputs, suitable input levels,
and simultaneous capture support before choosing a channel configuration.

## First analysis milestone

Start with activity, level and transients on recorded and live inputs. Then evaluate
beat/phase, bar inference, harmony and solo/section interpretation against rehearsal
recordings. This ordering is a proposal; harmony remains in project scope.

Each estimate needs a timestamp, confidence/freshness, and an unavailable state.
Silence after unplugging an interface must not masquerade as a musical ending.
Solo detection is not simply selecting the louder guitar.

## Multiple USB interfaces

Treat devices as independent clocks. Measure start offset and drift rather than assuming
matching sample rates mean sample-aligned streams. Decide whether timestamped features
are sufficient before adding sample-level alignment or resampling. Capture formats,
block sizes and latency budgets are still open.

Potential Rust backends to evaluate, not yet dependencies:
[CPAL](https://github.com/RustAudio/cpal) and [ALSA](https://www.alsa-project.org/wiki/Main_Page).

## Replay

Plan local multichannel recordings plus timestamped manual actions and decisions.
Keep recordings outside Git (`recordings/` is ignored). Small synthetic fixtures can
protect current production behavior; full rehearsal analysis and exhaustive algorithm
comparisons should be opt-in with recorded results.

Related: [architecture](0005-architecture.md), [validation](0012-development-and-validation.md).

[Notebook index](../index.md)

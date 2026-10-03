//! Eight virtual fixtures. A held composition, continuous motion and short accents
//! share one renderer; MIDI quantization belongs to the device backend.
use crate::{
    analysis::Snapshot,
    show::{Decision, Program, Scene},
};
use std::time::Duration;

pub const PAD_NAMES: [&str; 8] = [
    "L OUT", "L MID", "L IN", "L CORE", "R CORE", "R IN", "R MID", "R OUT",
];
pub type Rgb = [u8; 3];
pub type Pads = [Rgb; 8];
pub const OFF: Pads = [[0; 3]; 8];

pub struct Preview {
    program: Program,
    last: Option<Duration>,
    phase: f32,
    period: f32,
    age: f32,
    motif: usize,
    scene: Scene,
    palette: [Rgb; 2],
    from: Pads,
    current: Pads,
    base: Pads,
    transition: f32,
    kick_until: Duration,
    next_accent: Duration,
    accent_pair: usize,
    level: f32,
    guitar_balance: f32,
    burst_end: Option<Duration>,
    burst_start: Duration,
}
impl Default for Preview {
    fn default() -> Self {
        Self::new(Program::Metal)
    }
}
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let rgb: Rgb =
        std::array::from_fn(|i| (f32::from(a[i]) * (1.0 - t) + f32::from(b[i]) * t).round() as u8);
    // Remove the shared white component: white is reserved for bounded strobes.
    let common = *rgb.iter().min().unwrap();
    rgb.map(|v| v - common)
}
fn scale(a: Rgb, t: f32) -> Rgb {
    a.map(|v| (f32::from(v) * t.clamp(0.0, 1.0)).round() as u8)
}
fn colors(program: Program, scene: Scene, motif: usize) -> [Rgb; 2] {
    match (program, scene, motif % 2) {
        (Program::Atmospheric, _, _) => [[0, 0, 210], [0, 190, 210]],
        (Program::Punk, Scene::Intense, _) => [[210, 0, 0], [210, 180, 0]],
        (Program::Punk, _, 0) => [[0, 180, 0], [180, 180, 0]],
        (Program::Punk, _, _) => [[0, 0, 210], [0, 180, 210]],
        (Program::Metal, Scene::Intense, _) => [[210, 0, 0], [170, 0, 210]],
        (Program::Metal, _, 0) => [[0, 0, 210], [170, 0, 210]],
        (Program::Metal, _, _) => [[170, 0, 210], [210, 0, 0]],
    }
}
impl Preview {
    pub fn new(program: Program) -> Self {
        Self {
            program,
            last: None,
            phase: 0.0,
            period: 0.6,
            age: 0.0,
            motif: 0,
            scene: Scene::Calm,
            palette: colors(program, Scene::Calm, 0),
            from: OFF,
            current: OFF,
            base: OFF,
            transition: 0.0,
            kick_until: Duration::ZERO,
            next_accent: Duration::ZERO,
            accent_pair: 0,
            level: 0.0,
            guitar_balance: 0.0,
            burst_end: None,
            burst_start: Duration::ZERO,
        }
    }
    pub fn base(&self) -> Pads {
        self.base
    }
    pub fn look(&self) -> &'static str {
        if self.program == Program::Atmospheric {
            "Tidal wash"
        } else {
            [
                "Mirror sweep",
                "Crosscurrent",
                "Guitar conversation",
                "Center bloom",
            ][self.motif]
        }
    }
    pub fn update(&mut self, s: Snapshot, d: Decision) -> Pads {
        let gap = self.last.and_then(|at| s.at.checked_sub(at));
        if !s.reliable
            || !d.input_available
            || self.last.is_some() && gap.is_none_or(|dt| dt > Duration::from_millis(500))
        {
            *self = Self::new(self.program);
            return OFF;
        }
        let dt = gap.map_or(0.01, |dt| dt.as_secs_f32());
        self.last = Some(s.at);
        self.age += dt;
        // A regular pulse steers speed gradually. Never reset a chase on a kick;
        // double kicks cannot turn motion into an uncontrolled race.
        let target_period = if self.program == Program::Atmospheric {
            1.8
        } else {
            s.pulse
                .map_or(0.95 - 0.55 * s.energy, |p| 60.0 / p.bpm)
                .clamp(0.3, 1.0)
        };
        self.period += (target_period - self.period) * (dt / 2.0).min(1.0);
        self.phase = (self.phase + dt / self.period) % 16.0;
        let audible = s.active.iter().any(|a| *a);
        let target_level = if audible {
            0.25 + 0.5 * s.energy + 0.15 * s.normalized[1].min(1.0)
        } else {
            0.0
        };
        self.level += (target_level - self.level) * (dt / 0.6).min(1.0);
        self.guitar_balance += ((s.normalized[3] - s.normalized[2]).clamp(-1.0, 1.0)
            - self.guitar_balance)
            * (dt / 1.5).min(1.0);
        // Motifs live for a full program dwell. Sustained scene changes recolor
        // the composition without stealing phase or restarting its motion.
        if self.age >= self.program.minimum_scene().as_secs_f32() || self.scene != d.scene {
            self.from = self.current;
            self.transition = 0.0;
            if self.age >= self.program.minimum_scene().as_secs_f32() {
                self.motif = (self.motif + 1) % 4;
                self.age = 0.0;
            }
            self.scene = d.scene;
            self.palette = colors(self.program, d.scene, self.motif);
        }
        self.transition = (self.transition + dt / d.fade.as_secs_f32().max(0.1)).min(1.0);
        let mut target = OFF;
        for (i, pad) in target.iter_mut().enumerate() {
            let pair = i.min(7 - i) as f32;
            let side = if i < 4 { -1.0 } else { 1.0 };
            let position = if self.program == Program::Atmospheric {
                self.phase * 0.25 - pair * 0.35
            } else {
                match self.motif {
                    0 => self.phase - pair,
                    1 => self.phase + side * pair,
                    2 => self.phase * 0.5 - pair + side * self.guitar_balance,
                    _ => self.phase * 0.5 + pair,
                }
            };
            let width = 0.65 + 0.55 * s.normalized[1].clamp(0.0, 1.0);
            let wave = (1.0 - (position.rem_euclid(4.0) - 2.0).abs() / width).clamp(0.0, 1.0);
            let tint = mix(self.palette[0], self.palette[1], wave);
            *pad = scale(tint, self.level * (0.55 + 0.45 * wave));
        }
        self.current = std::array::from_fn(|i| mix(self.from[i], target[i], self.transition));
        // Small colored accents keep most fixtures and the underlying look intact.
        if s.kick && s.at >= self.next_accent && self.program != Program::Atmospheric {
            self.accent_pair = (self.accent_pair + 1) % 4;
            self.kick_until = s.at + Duration::from_millis(120);
            self.next_accent = s.at + Duration::from_millis(300);
        }
        self.base = self.current;
        if s.at < self.kick_until {
            for i in [self.accent_pair, 7 - self.accent_pair] {
                self.base[i] = mix(self.base[i], self.palette[1], 0.7);
            }
        }
        let mut pads = self.base;
        let burst = self.program != Program::Atmospheric
            && d.burst_requested
            && d.burst_expires_at.is_some_and(|end| s.at < end);
        if burst {
            if self.burst_end != d.burst_expires_at {
                self.burst_start = s.at;
                self.burst_end = d.burst_expires_at;
            }
            let elapsed = s.at.saturating_sub(self.burst_start).as_millis();
            // Three 80 ms flashes, 240 ms apart, one mirrored pair at a time.
            // Other fixtures retain color. Independent 720 ms renderer bound.
            if elapsed < 720 && elapsed % 240 < 80 {
                let pair = (elapsed / 240) as usize;
                pads[pair] = [255; 3];
                pads[7 - pair] = [255; 3];
            }
        } else {
            self.burst_end = None;
        }
        pads
    }
}

/// Finite, seeded lightning audition. Generate once; evaluate by absolute time so
/// stalled output skips missed flashes instead of replaying a queued sequence.
pub const WHITE_AUDITION_DURATION: Duration = Duration::from_secs(8);
#[derive(Clone, Copy, Default)]
struct Flash {
    start: u64,
    end: u64,
    pad: usize,
}
pub struct WhiteStorm {
    flashes: [Flash; 256],
    count: usize,
}
impl WhiteStorm {
    pub fn new(seed: u64) -> Self {
        // SplitMix64: repeatable auditions in tests, a fresh seed for each live run.
        let mut state = seed;
        let mut random = |limit: u64| {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            (z ^ (z >> 31)) % limit
        };
        let mut storm = Self {
            flashes: [Flash::default(); 256],
            count: 0,
        };
        let mut at = 150 + random(350);
        while at < 7500 && storm.count < 248 {
            let strikes = 1 + random(6);
            for _ in 0..strikes {
                let mask = if random(10) == 0 {
                    255
                } else {
                    1 + random(254)
                };
                for pad in 0..8 {
                    if mask & (1 << pad) != 0 && storm.count < 256 {
                        let start = at + random(35);
                        storm.flashes[storm.count] = Flash {
                            start,
                            end: start + 30 + random(80),
                            pad,
                        };
                        storm.count += 1;
                    }
                }
                at += 65 + random(190);
                if at >= 7500 {
                    break;
                }
            }
            // Unequal dark gaps separate ragged clusters; no shared beat/grid.
            at += 250 + random(650);
        }
        storm
    }
    pub fn frame(&self, at: Duration) -> Pads {
        if at >= WHITE_AUDITION_DURATION {
            return OFF;
        }
        let ms = at.as_millis();
        let mut pads = OFF;
        for flash in &self.flashes[..self.count] {
            if ms >= u128::from(flash.start) && ms < u128::from(flash.end) {
                pads[flash.pad] = [255; 3];
            }
        }
        pads
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(ms: u64) -> Snapshot {
        Snapshot {
            at: Duration::from_millis(ms),
            rms: [0.1; 4],
            normalized: [0.7; 4],
            active: [true; 4],
            kick: false,
            kick_strength: 0.0,
            kick_density: 3.0,
            pulse: None,
            energy: 0.7,
            slow_energy: 0.7,
            reliable: true,
        }
    }
    fn decision() -> Decision {
        Decision {
            scene: Scene::Drive,
            fade: Duration::from_secs(2),
            burst_requested: false,
            burst_expires_at: None,
            input_available: true,
        }
    }
    #[test]
    fn mirror_motion_survives_hardware_quantization_and_uses_all_pads() {
        let mut p = Preview::default();
        let mut colors = std::collections::BTreeSet::new();
        for ms in (0..12000).step_by(40) {
            let pads = p.update(snapshot(ms), decision());
            assert!((0..4).all(|i| pads[i] == pads[7 - i]));
            if ms > 4000 {
                assert!(pads.iter().all(|p| *p != [0; 3]));
                colors.insert(pads.map(crate::midi::color));
                assert!(!pads.iter().any(|p| crate::midi::color(*p) == 127));
            }
        }
        assert!(
            colors.len() >= 4,
            "motion must be visible with discrete LEDs"
        );
    }
    #[test]
    fn strobe_has_three_short_mirrored_flashes_and_retains_base() {
        let mut p = Preview::default();
        for ms in (0..2000).step_by(10) {
            p.update(snapshot(ms), decision());
        }
        let mut edges = 0;
        let mut was_white = false;
        for ms in (2000..3000).step_by(10) {
            let d = Decision {
                burst_requested: true,
                burst_expires_at: Some(Duration::from_millis(2750)),
                ..decision()
            };
            let pads = p.update(snapshot(ms), d);
            let whites = pads.iter().filter(|p| **p == [255; 3]).count();
            assert!(whites == 0 || whites == 2);
            if whites > 0 && !was_white {
                edges += 1;
            }
            was_white = whites > 0;
            assert!(
                pads.iter()
                    .filter(|p| **p != [255; 3] && **p != [0; 3])
                    .count()
                    >= 6
            );
            if ms >= 2750 {
                assert_eq!(pads, p.base());
            }
        }
        assert_eq!(edges, 3);
    }
    #[test]
    fn loss_cancels_motion_and_atmospheric_never_strobes() {
        let mut p = Preview::new(Program::Atmospheric);
        for ms in (0..1000).step_by(10) {
            let pads = p.update(
                snapshot(ms),
                Decision {
                    burst_requested: true,
                    burst_expires_at: Some(Duration::from_secs(2)),
                    ..decision()
                },
            );
            assert!(pads.iter().all(|p| *p != [255; 3]));
        }
        assert_eq!(
            p.update(
                Snapshot {
                    reliable: false,
                    ..snapshot(1000)
                },
                decision()
            ),
            OFF
        );
    }
    #[test]
    fn motifs_hold_and_silence_fades_out() {
        let mut p = Preview::new(Program::Punk);
        for ms in (0..17000).step_by(10) {
            p.update(snapshot(ms), decision());
            if ms < 15900 {
                assert_eq!(p.look(), "Mirror sweep");
            }
        }
        assert_eq!(p.look(), "Crosscurrent");
        for ms in (17000..24000).step_by(10) {
            p.update(
                Snapshot {
                    active: [false; 4],
                    normalized: [0.0; 4],
                    energy: 0.0,
                    ..snapshot(ms)
                },
                decision(),
            );
        }
        assert_eq!(p.base().map(crate::midi::color), [0; 8]);
    }
    #[test]
    fn palette_changes_and_accents_never_make_unrequested_white() {
        for program in Program::ALL {
            let mut p = Preview::new(program);
            for ms in (0..120000).step_by(40) {
                let mut s = snapshot(ms);
                s.kick = ms % 320 == 0;
                let pads = p.update(s, decision());
                assert!(!pads.iter().any(|p| crate::midi::color(*p) == 127));
            }
        }
    }
    #[test]
    fn bass_changes_spread_and_guitars_bias_the_conversation() {
        let mut left = Preview::default();
        let mut right = Preview::default();
        for ms in (0..54000).step_by(40) {
            let mut a = snapshot(ms);
            let mut b = a;
            a.normalized = [0.5, 0.2, 0.9, 0.1];
            b.normalized = [0.5, 0.9, 0.1, 0.9];
            left.update(a, decision());
            right.update(b, decision());
        }
        assert_eq!(left.look(), "Guitar conversation");
        assert_ne!(left.base(), right.base());
    }
    #[test]
    fn rapid_kicks_do_not_reset_chase_or_replace_the_whole_stage() {
        let mut accented = Preview::default();
        let mut plain = Preview::default();
        let mut accents = 0;
        let mut was_accent = false;
        for ms in (0..10000).step_by(10) {
            let s = snapshot(ms);
            let a = accented.update(
                Snapshot {
                    kick: ms % 100 == 0,
                    ..s
                },
                decision(),
            );
            let b = plain.update(s, decision());
            assert_eq!(accented.phase, plain.phase);
            assert!(a.iter().zip(b).filter(|(a, b)| **a != *b).count() <= 2);
            let on = Duration::from_millis(ms) < accented.kick_until;
            accents += usize::from(on && !was_accent);
            was_accent = on;
        }
        assert!(accents <= 34);
    }
    #[test]
    fn storm_is_scattered_irregular_repeatable_and_finite() {
        use std::collections::BTreeSet;
        let storm = WhiteStorm::new(42);
        let repeat = WhiteStorm::new(42);
        let other = WhiteStorm::new(43);
        let mut different = 0;
        let mut asymmetric = 0;
        let mut dark_run = 0;
        let mut longest_dark = 0;
        let mut widths = BTreeSet::new();
        for flash in &storm.flashes[..storm.count] {
            assert!((30..110).contains(&(flash.end - flash.start)));
            widths.insert(flash.end - flash.start);
        }
        assert!(widths.len() > 8);
        for ms in (0..9000).step_by(10) {
            let at = Duration::from_millis(ms);
            let frame = storm.frame(at);
            assert_eq!(frame, repeat.frame(at));
            different += usize::from(frame != other.frame(at));
            asymmetric += usize::from((0..4).any(|i| frame[i] != frame[7 - i]));
            if ms < 7500 {
                dark_run = if frame == OFF { dark_run + 10 } else { 0 };
                longest_dark = longest_dark.max(dark_run);
            }
            if ms >= 8000 {
                assert_eq!(frame, OFF);
            }
        }
        assert!(different > 50 && asymmetric > 50);
        assert!(longest_dark >= 250);
        // Random access does not consume or restart the sequence.
        assert_eq!(
            storm.frame(Duration::from_millis(400)),
            repeat.frame(Duration::from_millis(400))
        );
    }
}

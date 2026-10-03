//! Bounded, allocation-free show policy. Inputs are features, not raw audio.
//! Outputs are artistic intentions; this module has no fixture or hardware access.
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Program {
    Punk,
    Metal,
    Atmospheric,
}

impl Program {
    pub const ALL: [Self; 3] = [Self::Punk, Self::Metal, Self::Atmospheric];

    pub fn name(self) -> &'static str {
        match self {
            Self::Punk => "punk",
            Self::Metal => "metal",
            Self::Atmospheric => "atmospheric",
        }
    }

    pub fn minimum_scene(self) -> Duration {
        Duration::from_secs(match self {
            Self::Punk => 16,
            Self::Metal => 24,
            Self::Atmospheric => 48,
        })
    }

    pub fn fade(self) -> Duration {
        Duration::from_secs(match self {
            Self::Punk => 2,
            Self::Metal => 4,
            Self::Atmospheric => 8,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Calm,
    Drive,
    Intense,
}

/// Upstream must normalize energy against calibrated source levels and estimate
/// kick density separately from tempo. None at the API boundary means unavailable.
#[derive(Clone, Copy, Debug)]
pub struct Features {
    pub observed_at: Duration,
    pub energy: f32,
    pub kick_hits_per_second: f32,
    pub confidence: f32,
}

impl Features {
    fn valid(self, now: Duration) -> bool {
        now.checked_sub(self.observed_at)
            .is_some_and(|age| age <= FRESHNESS)
            && self.energy.is_finite()
            && (0.0..=1.0).contains(&self.energy)
            && self.kick_hits_per_second.is_finite()
            && (0.0..=40.0).contains(&self.kick_hits_per_second)
            && self.confidence.is_finite()
            && (0.8..=1.0).contains(&self.confidence)
    }
}

const FRESHNESS: Duration = Duration::from_millis(500);
const CONFIRM_SCENE: Duration = Duration::from_secs(2);
const CONFIRM_BURST: Duration = Duration::from_millis(600);
const BURST_LENGTH: Duration = Duration::from_millis(750);
const BURST_REST: Duration = Duration::from_secs(12);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub scene: Scene,
    /// Suggested transition duration; only consume when scene changes.
    pub fade: Duration,
    /// A bounded accent request, never a flash rate or a DMX channel value.
    pub burst_requested: bool,
    /// Absolute expiry on the caller's monotonic timeline; do not hold past this.
    pub burst_expires_at: Option<Duration>,
    pub input_available: bool,
}

/// Call with monotonic elapsed time at least every 500 ms, including during input
/// loss. A stalled caller or reversed clock cancels transient effects. Consumers
/// must independently expire output: stopping calls cannot switch off real fixtures.
pub struct Director {
    program: Program,
    scene: Scene,
    scene_since: Duration,
    candidate: Option<(Scene, Duration)>,
    dense_since: Option<Duration>,
    burst_until: Option<Duration>,
    next_burst: Duration,
    last_tick: Option<Duration>,
}

impl Director {
    pub fn new(program: Program) -> Self {
        Self {
            program,
            scene: Scene::Calm,
            scene_since: Duration::ZERO,
            candidate: None,
            dense_since: None,
            burst_until: None,
            next_burst: Duration::ZERO,
            last_tick: None,
        }
    }

    pub fn update(
        &mut self,
        now: Duration,
        features: Option<Features>,
        allow_bursts: bool,
    ) -> Decision {
        let discontinuity = self
            .last_tick
            .is_some_and(|last| now.checked_sub(last).is_none_or(|gap| gap > FRESHNESS));
        if self.last_tick.is_none() || discontinuity {
            self.scene_since = now;
            self.candidate = None;
            self.dense_since = None;
            self.burst_until = None;
            if discontinuity {
                self.next_burst = now.saturating_add(BURST_REST);
            }
        }
        self.last_tick = Some(now);
        let features = features.filter(|f| f.valid(now));
        if let Some(f) = features {
            // Hysteresis keeps levels near a boundary from changing the look.
            let wanted = match self.scene {
                Scene::Calm if f.energy >= 0.8 => Scene::Intense,
                Scene::Calm if f.energy >= 0.45 => Scene::Drive,
                Scene::Drive if f.energy >= 0.8 => Scene::Intense,
                Scene::Drive if f.energy < 0.3 => Scene::Calm,
                Scene::Intense if f.energy < 0.3 => Scene::Calm,
                Scene::Intense if f.energy < 0.65 => Scene::Drive,
                scene => scene,
            };
            if wanted == self.scene {
                self.candidate = None;
            } else {
                let (_, since) = *self.candidate.get_or_insert((wanted, now));
                if self.candidate.is_some_and(|(scene, _)| scene != wanted) {
                    self.candidate = Some((wanted, now));
                } else if now.saturating_sub(since) >= CONFIRM_SCENE
                    && now.saturating_sub(self.scene_since) >= self.program.minimum_scene()
                {
                    self.scene = wanted;
                    self.scene_since = now;
                    self.candidate = None;
                }
            }
            let dense = allow_bursts
                && self.program != Program::Atmospheric
                && f.energy >= 0.8
                && f.kick_hits_per_second >= 8.0;
            if dense {
                let since = *self.dense_since.get_or_insert(now);
                if now.saturating_sub(since) >= CONFIRM_BURST
                    && now >= self.next_burst
                    && self.burst_until.is_none()
                {
                    let until = now.saturating_add(BURST_LENGTH);
                    self.burst_until = Some(until);
                    self.next_burst = until.saturating_add(BURST_REST);
                }
            } else {
                self.dense_since = None;
                self.burst_until = None;
            }
        } else {
            // Hold the base scene on missing input; never infer silence from loss.
            self.candidate = None;
            self.dense_since = None;
            self.burst_until = None;
        }
        if self.burst_until.is_some_and(|until| now >= until) {
            self.burst_until = None;
        }
        Decision {
            scene: self.scene,
            fade: self.program.fade(),
            burst_requested: self.burst_until.is_some(),
            burst_expires_at: self.burst_until,
            input_available: features.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ms: u64, energy: f32, density: f32) -> Features {
        Features {
            observed_at: Duration::from_millis(ms),
            energy,
            kick_hits_per_second: density,
            confidence: 1.0,
        }
    }

    fn tick(d: &mut Director, ms: u64, energy: f32, density: f32, bursts: bool) -> Decision {
        d.update(
            Duration::from_millis(ms),
            Some(sample(ms, energy, density)),
            bursts,
        )
    }

    #[test]
    fn each_program_holds_its_scene_then_accepts_sustained_change() {
        for program in Program::ALL {
            let mut d = Director::new(program);
            let dwell = program.minimum_scene().as_millis() as u64;
            for ms in (0..dwell).step_by(100) {
                assert_eq!(tick(&mut d, ms, 0.9, 0.0, false).scene, Scene::Calm);
            }
            assert_eq!(tick(&mut d, dwell, 0.9, 0.0, false).scene, Scene::Intense);
        }
    }

    #[test]
    fn brief_fill_does_not_change_scene() {
        let mut d = Director::new(Program::Punk);
        for ms in (0..20_000).step_by(100) {
            tick(&mut d, ms, 0.1, 0.0, false);
        }
        for ms in (20_000..21_000).step_by(100) {
            assert_eq!(tick(&mut d, ms, 0.9, 12.0, false).scene, Scene::Calm);
        }
        assert_eq!(tick(&mut d, 21_000, 0.1, 0.0, false).scene, Scene::Calm);
    }

    #[test]
    fn hysteresis_preserves_intense_scene_near_entry_threshold() {
        let mut d = Director::new(Program::Punk);
        for ms in (0..17_000).step_by(100) {
            tick(&mut d, ms, 0.9, 0.0, false);
        }
        for ms in (17_000..40_000).step_by(100) {
            assert_eq!(tick(&mut d, ms, 0.75, 0.0, false).scene, Scene::Intense);
        }
    }

    #[test]
    fn dense_kicks_require_permission_energy_and_program() {
        for (program, energy, density, allow) in [
            (Program::Metal, 0.9, 12.0, false),
            (Program::Metal, 0.5, 12.0, true),
            (Program::Metal, 0.9, 4.0, true),
            (Program::Atmospheric, 0.9, 12.0, true),
        ] {
            let mut d = Director::new(program);
            for ms in (0..2000).step_by(100) {
                assert!(!tick(&mut d, ms, energy, density, allow).burst_requested);
            }
        }
    }

    #[test]
    fn burst_has_confirmation_deadline_and_rest_even_under_continuous_double_kick() {
        let mut d = Director::new(Program::Metal);
        for ms in (0..600).step_by(50) {
            assert!(!tick(&mut d, ms, 0.9, 12.0, true).burst_requested);
        }
        for ms in (600..1350).step_by(50) {
            assert!(tick(&mut d, ms, 0.9, 12.0, true).burst_requested);
        }
        for ms in (1350..13_350).step_by(50) {
            assert!(!tick(&mut d, ms, 0.9, 12.0, true).burst_requested);
        }
        assert!(tick(&mut d, 13_350, 0.9, 12.0, true).burst_requested);
    }

    #[test]
    fn missing_stale_future_invalid_or_uncertain_features_cancel_bursts() {
        let mut invalid = vec![
            None,
            Some(sample(0, 0.9, 12.0)),
            Some(sample(900, 0.9, 12.0)),
        ];
        for (energy, density, confidence) in [
            (f32::NAN, 12.0, 1.0),
            (1.1, 12.0, 1.0),
            (0.9, f32::INFINITY, 1.0),
            (0.9, -1.0, 1.0),
            (0.9, 12.0, f32::NAN),
            (0.9, 12.0, 0.7),
        ] {
            invalid.push(Some(Features {
                confidence,
                ..sample(700, energy, density)
            }));
        }
        for f in invalid {
            let mut d = Director::new(Program::Metal);
            for ms in (0..=600).step_by(100) {
                tick(&mut d, ms, 0.9, 12.0, true);
            }
            let out = d.update(Duration::from_millis(700), f, true);
            assert!(!out.burst_requested);
            assert!(!out.input_available);
        }
    }

    #[test]
    fn disabling_bursts_cancels_immediately_and_does_not_clear_rest() {
        let mut d = Director::new(Program::Metal);
        for ms in (0..=600).step_by(100) {
            tick(&mut d, ms, 0.9, 12.0, true);
        }
        assert!(!tick(&mut d, 700, 0.9, 12.0, false).burst_requested);
        for ms in (800..3000).step_by(100) {
            assert!(!tick(&mut d, ms, 0.9, 12.0, true).burst_requested);
        }
    }

    #[test]
    fn stall_and_clock_reversal_discard_transient_history() {
        for next in [300, 2000] {
            let mut d = Director::new(Program::Metal);
            for ms in (0..=600).step_by(100) {
                tick(&mut d, ms, 0.9, 12.0, true);
            }
            assert!(!tick(&mut d, next, 0.9, 12.0, true).burst_requested);
        }
    }

    #[test]
    fn input_loss_holds_scene_and_recovery_requires_new_confirmation() {
        let mut d = Director::new(Program::Punk);
        for ms in (0..40_000).step_by(100) {
            tick(&mut d, ms, 0.9, 0.0, false);
        }
        for ms in (40_000..42_000).step_by(100) {
            assert_eq!(
                d.update(Duration::from_millis(ms), None, false).scene,
                Scene::Intense
            );
        }
        for ms in (42_000..44_000).step_by(100) {
            assert_eq!(tick(&mut d, ms, 0.1, 0.0, false).scene, Scene::Intense);
        }
        assert_eq!(tick(&mut d, 44_000, 0.1, 0.0, false).scene, Scene::Calm);
    }
}

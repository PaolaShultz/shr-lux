//! Small causal analyzer for isolated kick/bass/two-guitar feeds at 48 kHz.
//! No FFT, model inference, hardware access, or unbounded history.
use crate::{
    replay::{AuxFrame, SAMPLE_RATE},
    show::Features,
};
use std::time::Duration;

pub const WINDOW: usize = 480; // 10 ms; all temporal constants are defined at this rate.
const DT: f32 = WINDOW as f32 / SAMPLE_RATE as f32;

#[derive(Clone, Copy, Debug)]
pub struct Calibration {
    /// 90th-percentile non-silent 10 ms RMS, one reference per source.
    pub reference: [f32; 4],
}
impl Calibration {
    pub fn new(reference: [f32; 4]) -> Result<Self, &'static str> {
        if reference
            .iter()
            .any(|x| !x.is_finite() || *x < 0.0001 || *x > 1.0)
        {
            return Err("Calibration RMS references must be finite and in 0.0001..=1");
        }
        Ok(Self { reference })
    }
}

/// Bounded level histogram. For file simulation this is an explicit soundcheck
/// pass over the file. The causal analyzer never reads future samples.
#[derive(Clone)]
pub struct Calibrator {
    bins: [[u32; 81]; 4],
}
impl Default for Calibrator {
    fn default() -> Self {
        Self { bins: [[0; 81]; 4] }
    }
}
impl Calibrator {
    pub fn observe(&mut self, frames: &[AuxFrame]) {
        if frames.is_empty() {
            return;
        }
        let rms = levels(frames);
        for (bins, value) in self.bins.iter_mut().zip(rms) {
            if value.is_finite() && value >= 0.0001 {
                let bin = (20.0 * value.log10() + 80.0).clamp(0.0, 80.0) as usize;
                bins[bin] = bins[bin].saturating_add(1);
            }
        }
    }
    pub fn finish(&self) -> Calibration {
        let reference = self.bins.map(|bins| {
            let total: u64 = bins.iter().map(|x| u64::from(*x)).sum();
            if total == 0 {
                return 0.01;
            }
            let target = (total * 9).div_ceil(10);
            let mut sum = 0;
            for (index, count) in bins.iter().enumerate() {
                sum += u64::from(*count);
                if sum >= target {
                    return 10.0_f32.powf((index as f32 + 1.0 - 80.0) / 20.0).min(1.0);
                }
            }
            1.0
        });
        Calibration { reference }
    }
}

fn levels(frames: &[AuxFrame]) -> [f32; 4] {
    let mut power = [0.0; 4];
    for frame in frames {
        for (sum, sample) in power.iter_mut().zip(frame) {
            *sum += sample * sample;
        }
    }
    power.map(|sum| (sum / frames.len() as f32).sqrt())
}

#[derive(Clone, Copy, Debug)]
pub struct Pulse {
    /// Regular kick pulse folded to 60–240 BPM. Not a verified musical beat/bar.
    pub bpm: f32,
    pub regularity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub at: Duration,
    pub rms: [f32; 4],
    pub normalized: [f32; 4],
    pub active: [bool; 4],
    pub kick: bool,
    pub kick_strength: f32,
    pub kick_density: f32,
    pub pulse: Option<Pulse>,
    pub energy: f32,
    pub slow_energy: f32,
    pub reliable: bool,
}
impl Snapshot {
    pub fn features(self) -> Option<Features> {
        self.reliable.then_some(Features {
            observed_at: self.at,
            energy: self.energy,
            kick_hits_per_second: self.kick_density,
            confidence: 1.0,
        })
    }
}

#[derive(Clone)]
pub struct Analyzer {
    calibration: Calibration,
    smooth: [f32; 4],
    active: [bool; 4],
    release: [f32; 4],
    kick_low: f32,
    kick_dc: f32,
    previous_attack: f32,
    baseline: f32,
    last_hit: Option<Duration>,
    hits: [Option<Duration>; 64],
    hit_cursor: usize,
    intervals: [f32; 12],
    interval_count: usize,
    interval_cursor: usize,
    energy: f32,
    slow_energy: f32,
    expected_frame: Option<u32>,
    warmup: f32,
}
impl Analyzer {
    pub fn new(calibration: Calibration) -> Self {
        Self {
            calibration,
            smooth: [0.0; 4],
            active: [false; 4],
            release: [0.0; 4],
            kick_low: 0.0,
            kick_dc: 0.0,
            previous_attack: 0.0,
            baseline: 0.0,
            last_hit: None,
            hits: [None; 64],
            hit_cursor: 0,
            intervals: [0.0; 12],
            interval_count: 0,
            interval_cursor: 0,
            energy: 0.0,
            slow_energy: 0.0,
            expected_frame: None,
            warmup: 0.0,
        }
    }

    /// Supply consecutive, full 10 ms windows. A discontinuity resets history and
    /// starts a 500 ms settling period. Invalid/clipped input inhibits decisions.
    /// The final short file window may be ignored by the caller (less than 10 ms).
    pub fn process(
        &mut self,
        first_frame: u32,
        frames: &[AuxFrame],
    ) -> Result<Snapshot, &'static str> {
        if frames.len() != WINDOW {
            return Err("Analysis requires a 480-frame window");
        }
        if frames
            .iter()
            .flatten()
            .any(|x| !x.is_finite() || x.abs() > 1.0)
        {
            *self = Self::new(self.calibration);
            return Err("Non-finite or out-of-range PCM");
        }
        if self
            .expected_frame
            .is_some_and(|expected| expected != first_frame)
        {
            *self = Self::new(self.calibration);
        }
        let end_frame = first_frame
            .checked_add(WINDOW as u32)
            .ok_or("Sample clock overflow")?;
        self.expected_frame = Some(end_frame);
        let at = crate::replay::frame_time(end_frame);
        let rms = levels(frames);
        let clipped = frames
            .iter()
            .filter(|f| f.iter().any(|x| x.abs() >= 0.999))
            .count()
            > 2;
        if clipped {
            self.warmup = 0.0;
        }
        self.warmup += DT;
        let reliable = self.warmup >= 0.5 && !clipped;
        for (i, value) in rms.iter().enumerate() {
            let normalized = (*value / self.calibration.reference[i]).min(1.5);
            self.smooth[i] += 0.1 * (normalized - self.smooth[i]);
            let gate = if self.active[i] { 0.06 } else { 0.12 };
            if self.smooth[i] > gate && *value > 0.0001 {
                self.active[i] = true;
                self.release[i] = 0.0;
            } else {
                self.release[i] += DT;
                if self.release[i] >= 0.25 {
                    self.active[i] = false;
                }
            }
        }
        // Kick body envelope after DC removal and a ~200 Hz single-pole lowpass.
        let mut low_power = 0.0;
        for frame in frames {
            self.kick_dc += 0.0026 * (frame[0] - self.kick_dc); // ~20 Hz DC/highpass state
            self.kick_low += 0.026 * (frame[0] - self.kick_dc - self.kick_low);
            low_power += self.kick_low * self.kick_low;
        }
        let body = (low_power / WINDOW as f32).sqrt() / self.calibration.reference[0];
        // Isolated click-heavy kicks can still contribute via the broadband RMS.
        let attack = 0.7 * body + 0.3 * rms[0] / self.calibration.reference[0];
        let rise = (attack - self.previous_attack).max(0.0);
        let kick = reliable
            && attack > 0.12
            && rise > 0.12
            && attack > self.baseline * 1.35
            && self
                .last_hit
                .is_none_or(|last| at.saturating_sub(last) >= Duration::from_millis(60));
        self.previous_attack = attack;
        self.baseline += 0.05 * (attack - self.baseline);
        if kick {
            if let Some(last) = self.last_hit {
                let interval = at.saturating_sub(last).as_secs_f32();
                if (0.06..=2.0).contains(&interval) {
                    self.intervals[self.interval_cursor] = interval;
                    self.interval_cursor = (self.interval_cursor + 1) % self.intervals.len();
                    self.interval_count = (self.interval_count + 1).min(self.intervals.len());
                } else {
                    self.interval_count = 0;
                    self.interval_cursor = 0;
                }
            }
            self.last_hit = Some(at);
            self.hits[self.hit_cursor] = Some(at);
            self.hit_cursor = (self.hit_cursor + 1) % self.hits.len();
        }
        let kick_density = self
            .hits
            .iter()
            .filter(|hit| hit.is_some_and(|t| at.saturating_sub(t) < Duration::from_secs(1)))
            .count() as f32;
        let pulse = self.pulse(at);
        // Source-normalized sustained activity plus a restrained rhythm contribution.
        let instruments = (self.smooth[1] + self.smooth[2] + self.smooth[3]) / 3.0;
        let target = (0.85 * instruments + 0.15 * (kick_density / 8.0).min(1.0)).clamp(0.0, 1.0);
        self.energy += 0.02 * (target - self.energy); // about 500 ms
        self.slow_energy += 0.0025 * (self.energy - self.slow_energy); // about 4 s
        if !reliable {
            self.hits = [None; 64];
            self.last_hit = None;
            self.interval_count = 0;
            self.interval_cursor = 0;
        }
        Ok(Snapshot {
            at,
            rms,
            normalized: self.smooth.map(|x| x.clamp(0.0, 1.0)),
            active: self.active,
            kick,
            kick_strength: if kick { rise.min(1.0) } else { 0.0 },
            kick_density: if reliable { kick_density } else { 0.0 },
            pulse: if reliable { pulse } else { None },
            energy: self.energy,
            slow_energy: self.slow_energy,
            reliable,
        })
    }

    fn pulse(&self, at: Duration) -> Option<Pulse> {
        if self.interval_count < 6
            || self
                .last_hit
                .is_none_or(|t| at.saturating_sub(t) > Duration::from_secs(2))
        {
            return None;
        }
        let mut sorted = self.intervals;
        sorted[..self.interval_count].sort_by(f32::total_cmp);
        let median = sorted[self.interval_count / 2];
        let consistent = sorted[..self.interval_count]
            .iter()
            .filter(|x| (**x - median).abs() < median * 0.12)
            .count();
        let regularity = consistent as f32 / self.interval_count as f32;
        if regularity < 0.7 {
            return None;
        }
        let mut period = median;
        while period < 0.25 {
            period *= 2.0;
        }
        while period > 1.0 {
            period *= 0.5;
        }
        Some(Pulse {
            bpm: 60.0 / period,
            regularity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn analyzer() -> Analyzer {
        Analyzer::new(Calibration::new([0.15; 4]).unwrap())
    }
    fn kicks(seconds: usize, period: f32) -> Vec<Snapshot> {
        let mut a = analyzer();
        let mut output = Vec::new();
        for window in 0..seconds * 100 {
            let first = window * WINDOW;
            let mut frames = [[0.0; 4]; WINDOW];
            for (i, f) in frames.iter_mut().enumerate() {
                let t = (first + i) as f32 / SAMPLE_RATE as f32;
                let phase = t % period;
                let env = if t >= 1.0 && phase < 0.12 {
                    (-phase / 0.025).exp()
                } else {
                    0.0
                };
                f[0] = 0.6 * env * (TAU * 65.0 * t).sin();
            }
            output.push(a.process(first as u32, &frames).unwrap());
        }
        output
    }
    #[test]
    fn silence_has_no_activity_hits_or_tempo() {
        let mut a = analyzer();
        for i in 0..1000 {
            let s = a.process(i * WINDOW as u32, &[[0.0; 4]; WINDOW]).unwrap();
            assert!(!s.kick && s.active == [false; 4] && s.energy == 0.0 && s.pulse.is_none());
        }
    }
    #[test]
    fn detects_regular_kicks_once_and_estimates_pulse() {
        let result = kicks(10, 0.5);
        let hits: Vec<_> = result.iter().filter(|s| s.kick).collect();
        assert!((17..=19).contains(&hits.len()), "{} hits", hits.len());
        assert!(
            hits.windows(2)
                .all(|pair| pair[1].at - pair[0].at > Duration::from_millis(400))
        );
        let pulse = result.last().unwrap().pulse.unwrap();
        assert!((pulse.bpm - 120.0).abs() < 3.0);
    }
    #[test]
    fn retains_fast_double_kicks() {
        let result = kicks(5, 0.1);
        let count = result.iter().filter(|s| s.kick).count();
        assert!((38..=41).contains(&count), "{count} hits");
        assert!(result.iter().any(|s| s.kick_density >= 9.0));
    }
    #[test]
    fn activity_releases_and_cannot_be_mislabelled_as_a_solo() {
        let mut a = analyzer();
        let mut s = None;
        for i in 0..100 {
            s = Some(
                a.process(i * WINDOW as u32, &[[0.0, 0.0, 0.1, 0.0]; WINDOW])
                    .unwrap(),
            );
        }
        assert_eq!(s.unwrap().active, [false, false, true, false]);
        for i in 100..300 {
            s = Some(a.process(i * WINDOW as u32, &[[0.0; 4]; WINDOW]).unwrap());
        }
        assert_eq!(s.unwrap().active, [false; 4]);
    }
    #[test]
    fn gaps_clipping_and_invalid_pcm_inhibit_decisions() {
        let mut a = analyzer();
        for i in 0..100 {
            a.process(i * WINDOW as u32, &[[0.1; 4]; WINDOW]).unwrap();
        }
        assert!(
            !a.process(200 * WINDOW as u32, &[[0.1; 4]; WINDOW])
                .unwrap()
                .reliable
        );
        assert!(
            !a.process(201 * WINDOW as u32, &[[1.0; 4]; WINDOW])
                .unwrap()
                .reliable
        );
        assert!(
            a.process(202 * WINDOW as u32, &[[f32::NAN; 4]; WINDOW])
                .is_err()
        );
        assert!(a.process(203 * WINDOW as u32, &[[0.0; 4]; 10]).is_err());
    }
    #[test]
    fn calibration_is_bounded_and_preserves_relative_dynamics() {
        let mut c = Calibrator::default();
        for _ in 0..100 {
            c.observe(&[[0.01, 0.05, 0.1, 0.2]; WINDOW]);
        }
        let reference = c.finish().reference;
        for (measured, expected) in reference.into_iter().zip([0.01, 0.05, 0.1, 0.2]) {
            assert!((measured / expected - 1.0).abs() < 0.13);
        }
        assert!(Calibration::new([f32::NAN; 4]).is_err());
    }
}

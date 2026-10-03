//! Recorded-input analysis and show integration. Offline and paced runs use the
//! same sample clock and DSP. Source tempo metadata is never used by the analyzer.
use crate::{
    analysis::{Analyzer, Calibration, Calibrator, Snapshot, WINDOW},
    preview::{Pads, Preview},
    replay::AuxReplay,
    show::{Decision, Director, Program},
};
use std::{
    error::Error,
    fs::File,
    io::{self, BufReader, Read, Seek, Write},
    path::Path,
    time::Duration,
};

#[derive(Clone, Copy)]
pub struct State {
    pub music: Snapshot,
    pub show: Decision,
    pub pads: Pads,
    pub base_pads: Pads,
    pub look: &'static str,
}

pub struct Session<R: Read + Seek> {
    replay: AuxReplay<R>,
    analyzer: Analyzer,
    director: Director,
    preview: Preview,
    buffer: [[f32; 4]; WINDOW],
    pub calibration: Calibration,
    pub kicks: u32,
}
impl<R: Read + Seek> Session<R> {
    pub fn new(mut replay: AuxReplay<R>, program: Program) -> Result<Self, Box<dyn Error>> {
        let mut calibration = Calibrator::default();
        let mut buffer = [[0.0; 4]; WINDOW];
        loop {
            let block = replay.read_block(&mut buffer)?;
            if block.frames == 0 {
                break;
            }
            calibration.observe(&buffer[..block.frames]);
        }
        replay.seek(0)?;
        let calibration = calibration.finish();
        Ok(Self {
            replay,
            analyzer: Analyzer::new(calibration),
            director: Director::new(program),
            preview: Preview::new(program),
            buffer,
            calibration,
            kicks: 0,
        })
    }
    pub fn seek(&mut self, frame: u32, program: Program) -> Result<(), Box<dyn Error>> {
        if frame >= self.replay.total_frames() {
            return Err("Start must be before EOF".into());
        }
        self.replay.seek(frame)?;
        self.analyzer = Analyzer::new(self.calibration);
        self.director = Director::new(program);
        self.preview = Preview::new(program);
        self.kicks = 0;
        Ok(())
    }
    pub fn duration(&self) -> Duration {
        crate::replay::frame_time(self.replay.total_frames())
    }
    pub fn next(&mut self, bursts: bool) -> Result<Option<State>, Box<dyn Error>> {
        let block = self.replay.read_block(&mut self.buffer)?;
        if block.frames < WINDOW {
            return Ok(None);
        }
        let music = self.analyzer.process(block.first_frame, &self.buffer)?;
        if music.kick {
            self.kicks += 1;
        }
        let show = self.director.update(music.at, music.features(), bursts);
        let pads = self.preview.update(music, show);
        Ok(Some(State {
            music,
            show,
            pads,
            base_pads: self.preview.base(),
            look: self.preview.look(),
        }))
    }
}
pub fn open(path: &Path, program: Program) -> Result<Session<BufReader<File>>, Box<dyn Error>> {
    Session::new(AuxReplay::new(BufReader::new(File::open(path)?))?, program)
}
pub fn analyze(path: &Path, program: Program, bursts: bool) -> Result<(), Box<dyn Error>> {
    let mut session = open(path, program)?;
    eprintln!(
        "File soundcheck RMS references: {:?}; no hardware output",
        session.calibration.reference
    );
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "time_s,kick,kick_strength,kicks_per_s,energy,slow_energy,active_mask,pulse_bpm,pulse_regularity,reliable,scene,burst,kick_rms,bass_rms,guitar1_rms,guitar2_rms"
    )?;
    while let Some(s) = session.next(bursts)? {
        let m = s.music;
        let mask = m
            .active
            .iter()
            .enumerate()
            .fold(0, |n, (i, on)| n | ((*on as u8) << i));
        let bpm = m.pulse.map(|p| format!("{:.2}", p.bpm)).unwrap_or_default();
        let regularity = m
            .pulse
            .map(|p| format!("{:.2}", p.regularity))
            .unwrap_or_default();
        writeln!(
            output,
            "{:.3},{},{:.3},{:.1},{:.3},{:.3},{},{},{},{},{:?},{},{:.6},{:.6},{:.6},{:.6}",
            m.at.as_secs_f64(),
            u8::from(m.kick),
            m.kick_strength,
            m.kick_density,
            m.energy,
            m.slow_energy,
            mask,
            bpm,
            regularity,
            u8::from(m.reliable),
            s.show.scene,
            u8::from(s.show.burst_requested),
            m.rms[0],
            m.rms[1],
            m.rms[2],
            m.rms[3]
        )?;
    }
    output.flush()?;
    eprintln!(
        "Detected {} kick candidates. Pulse tempo remains half/double ambiguous.",
        session.kicks
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{f32::consts::TAU, io::Cursor};

    #[test]
    fn audio_drives_pads_and_bounded_burst_then_silence_releases() {
        let mut bytes = Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: 4,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wav = hound::WavWriter::new(&mut bytes, spec).unwrap();
        for sample in 0..48000 * 8 {
            let t = sample as f32 / 48000.0;
            let phase = (sample % 4800) as f32 / 48000.0;
            let active = (1.0..6.0).contains(&t);
            for channel in 0..4 {
                let value = if !active {
                    0.0
                } else if channel == 0 {
                    0.6 * (-phase / 0.025).exp() * (TAU * 65.0 * t).sin()
                } else {
                    0.2 * (TAU * (110.0 * channel as f32) * t).sin()
                };
                wav.write_sample((value * 32767.0) as i16).unwrap();
            }
        }
        wav.finalize().unwrap();
        bytes.set_position(0);
        let mut session = Session::new(AuxReplay::new(bytes).unwrap(), Program::Metal).unwrap();
        let mut burst_frames = 0;
        let mut kick_pad = false;
        let mut white_frames = 0;
        let mut last = None;
        while let Some(s) = session.next(true).unwrap() {
            if s.show.burst_requested {
                burst_frames += 1;
                assert!(s.pads.iter().filter(|p| **p == [255; 3]).count() <= 2);
            }
            white_frames += usize::from(s.pads.contains(&[255; 3]));
            kick_pad |= s.pads[0] != [0; 3];
            last = Some(s);
        }
        assert!(kick_pad && session.kicks >= 45);
        assert!((1..=24).contains(&white_frames));
        assert!(
            (1..=75).contains(&burst_frames),
            "{burst_frames} burst frames"
        );
        assert_eq!(last.unwrap().music.active, [false; 4]);
        assert!(!last.unwrap().show.burst_requested);
        session.seek(0, Program::Metal).unwrap();
        while let Some(s) = session.next(false).unwrap() {
            assert!(!s.show.burst_requested);
            assert_eq!(s.pads, s.base_pads);
        }
    }
}

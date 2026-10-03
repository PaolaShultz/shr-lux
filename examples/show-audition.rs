//! Opt-in full-song renderer audit. No MIDI device is opened.
use shr_lux::{midi, show::Program, simulation};
use std::{collections::BTreeSet, error::Error, path::Path};
fn main() -> Result<(), Box<dyn Error>> {
    for program in [Program::Punk, Program::Metal] {
        let path = format!("recordings/simulation/{}/aux.wav", program.name());
        let mut session = simulation::open(Path::new(&path), program)?;
        let mut frames = BTreeSet::new();
        let mut looks = BTreeSet::new();
        let mut previous = [0; 8];
        let mut changes = 0;
        let mut flashes = 0;
        let mut white = false;
        let mut tick = 0;
        while let Some(state) = session.next(true)? {
            if tick % 4 == 0 {
                let colors = state.pads.map(midi::color);
                let whites = colors.iter().filter(|c| **c == 127).count();
                assert!(whites <= 2, "full-stage white is forbidden");
                assert!(whites == 0 || state.show.burst_requested);
                flashes += usize::from(whites > 0 && !white);
                white = whites > 0;
                changes += usize::from(colors != previous);
                previous = colors;
                frames.insert(colors);
                looks.insert(state.look);
            }
            tick += 1;
        }
        assert!(frames.len() > 10 && changes > 100 && looks.len() == 4);
        println!(
            "{}: {} distinct device frames, {} changes, {} looks, {} white pulses",
            program.name(),
            frames.len(),
            changes,
            looks.len(),
            flashes
        );
    }
    Ok(())
}

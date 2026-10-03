//! Stream prepared AUX files without a sound card. Print measured levels as CSV.
use shr_lux::replay::{AuxReplay, SAMPLE_RATE, frame_time};
use std::{
    fs::File,
    io::{self, BufReader, Write},
    time::Instant,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        return help();
    };
    if path == "--help" {
        return help();
    }
    let mut realtime = false;
    let mut start = 0.0;
    let mut seconds = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--realtime" => realtime = true,
            "--start" => start = number(args.next())?,
            "--seconds" => seconds = Some(number(args.next())?),
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
    }
    let mut replay = AuxReplay::new(BufReader::new(File::open(path)?))?;
    let start_frame = (start * f64::from(SAMPLE_RATE)).floor() as u32;
    if start >= frame_time(replay.total_frames()).as_secs_f64() {
        return Err("Start must be before the end of the song".into());
    }
    replay.seek(start_frame)?;
    let end = seconds.map_or(replay.total_frames(), |s| {
        start_frame
            .saturating_add((s * f64::from(SAMPLE_RATE)).floor() as u32)
            .min(replay.total_frames())
    });
    if end <= start_frame {
        return Err("Requested duration is shorter than one sample".into());
    }
    eprintln!(
        "FILE SIMULATION | kick,bass,guitar_1,guitar_2 | OUTPUT DISABLED\nMeasured RMS only; no kick, tempo or scene detector is connected."
    );
    let mut buffer = [[0.0; 4]; 4800]; // 100 ms; independent of total song length.
    let wall_start = Instant::now();
    let mut position = start_frame;
    let mut out = io::BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "time_s,frames,kick_dbfs,bass_dbfs,guitar_1_dbfs,guitar_2_dbfs"
    )?;
    while position < end {
        let capacity = buffer.len().min((end - position) as usize);
        let block = replay.read_block(&mut buffer[..capacity])?;
        if block.frames == 0 {
            break;
        }
        position += block.frames as u32;
        // Pacing follows one absolute sample clock, not repeated relative sleeps.
        if realtime {
            let due = frame_time(position - start_frame);
            if let Some(wait) = due.checked_sub(wall_start.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        let mut sum = [0.0_f64; 4];
        for frame in &buffer[..block.frames] {
            for (sum, sample) in sum.iter_mut().zip(frame) {
                *sum += f64::from(*sample).powi(2);
            }
        }
        let db = sum.map(|s| {
            if s == 0.0 {
                f64::NEG_INFINITY
            } else {
                10.0 * (s / block.frames as f64).log10()
            }
        });
        writeln!(
            out,
            "{:.6},{},{:.2},{:.2},{:.2},{:.2}",
            block.timestamp().as_secs_f64(),
            block.frames,
            db[0],
            db[1],
            db[2],
            db[3]
        )?;
        if realtime {
            out.flush()?;
        }
    }
    out.flush()?;
    Ok(())
}

fn number(value: Option<String>) -> Result<f64, Box<dyn std::error::Error>> {
    let value: f64 = value.ok_or("Missing seconds value")?.parse()?;
    if !value.is_finite() || !(0.0..=86_400.0).contains(&value) {
        return Err("Seconds must be finite and between 0 and 86400".into());
    }
    Ok(value)
}

fn help() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Usage: cargo run --locked --example aux-replay -- <aux.wav> [--realtime] [--start SECONDS] [--seconds SECONDS]\nReads four synchronized AUX channels. Default: offline CSV levels.\n--realtime: pace to the sample clock; no speaker playback, no DMX.\nPrepare songs with python3 scripts/prepare-simulation.py."
    );
    Ok(())
}

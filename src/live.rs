//! Paced file simulation with terminal/pad previews. No DMX transport.
use crate::{
    preview::{OFF, PAD_NAMES},
    show::Program,
    simulation::{self, State},
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Paragraph},
};
use std::{
    error::Error,
    io::{self, IsTerminal},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub struct Options {
    pub program: Program,
    pub file: PathBuf,
    pub bursts: bool,
    pub midi: bool,
    pub headless: bool,
    pub listen: bool,
    pub start: f64,
    pub seconds: Option<f64>,
}
impl Options {
    pub fn parse(program: &str, args: &[String]) -> Result<Self, Box<dyn Error>> {
        let program = Program::ALL
            .into_iter()
            .find(|p| p.name() == program)
            .ok_or("Expected punk, metal or atmospheric")?;
        let source = if program == Program::Atmospheric {
            "metal"
        } else {
            program.name()
        };
        let mut result = Self {
            program,
            file: format!("recordings/simulation/{source}/aux.wav").into(),
            bursts: false,
            midi: false,
            headless: false,
            listen: false,
            start: 0.0,
            seconds: None,
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--file" => result.file = args.next().ok_or("Missing WAV path")?.into(),
                "--bursts" => result.bursts = true,
                "--midi" => result.midi = true,
                "--headless" => result.headless = true,
                "--listen" => result.listen = true,
                "--start" => result.start = number(args.next())?,
                "--seconds" => result.seconds = Some(number(args.next())?),
                _ => return Err(format!("Unknown simulation option: {arg}").into()),
            }
        }
        if result.seconds.is_some_and(|n| n < 0.01) {
            return Err("Duration must be at least 0.01 seconds".into());
        }
        Ok(result)
    }
}
fn number(value: Option<&String>) -> Result<f64, Box<dyn Error>> {
    let value: f64 = value.ok_or("Missing time value")?.parse()?;
    if !value.is_finite() || !(0.0..=86400.0).contains(&value) {
        return Err("Time must be finite and between 0 and 86400 seconds".into());
    }
    Ok(value)
}

pub(crate) struct Stop {
    pub flag: Arc<AtomicBool>,
    ids: Vec<signal_hook::SigId>,
}
impl Stop {
    pub(crate) fn new() -> io::Result<Self> {
        let mut result = Self {
            flag: Arc::new(AtomicBool::new(false)),
            ids: Vec::new(),
        };
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            result.ids.push(signal_hook::flag::register(
                signal,
                Arc::clone(&result.flag),
            )?);
        }
        Ok(result)
    }
}
impl Drop for Stop {
    fn drop(&mut self) {
        for id in &self.ids {
            signal_hook::low_level::unregister(*id);
        }
    }
}

struct Playback(Child);
impl Playback {
    fn start(options: &Options) -> Result<Self, Box<dyn Error>> {
        let monitor = options.file.with_file_name("monitor.wav");
        if options.file.file_name().and_then(|s| s.to_str()) != Some("aux.wav")
            || !monitor.is_file()
        {
            return Err("--listen requires a prepared aux.wav and its sibling monitor.wav".into());
        }
        let mut command = Command::new("play");
        command
            .arg("-q")
            .arg(&monitor)
            .arg("trim")
            .arg(options.start.to_string());
        if let Some(seconds) = options.seconds {
            command.arg(seconds.to_string());
        }
        Ok(Self(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        ))
    }
    fn check(&mut self) -> io::Result<()> {
        if let Some(status) = self.0.try_wait()?
            && !status.success()
        {
            return Err(io::Error::other(
                "Monitor playback failed; check the default sound output with play monitor.wav",
            ));
        }
        Ok(())
    }
}
impl Drop for Playback {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn run(mut options: Options) -> Result<(), Box<dyn Error>> {
    if !options.headless && (!io::stdin().is_terminal() || !io::stdout().is_terminal()) {
        return Err(
            "Simulation TUI requires a terminal; use --headless for a timed console run".into(),
        );
    }
    let stop = Stop::new()?;
    eprintln!("Calibrating source levels from the recording (bounded soundcheck pass)...");
    let mut session = simulation::open(&options.file, options.program)?;
    if stop.flag.load(Ordering::Relaxed) {
        return Ok(());
    }
    let first_frame = (options.start * 48000.0).floor() as u32;
    session.seek(first_frame, options.program)?;
    let start = crate::replay::frame_time(first_frame);
    let end = options.seconds.map_or(session.duration(), |seconds| {
        (start + Duration::from_secs_f64(seconds)).min(session.duration())
    });
    let feedback = if options.midi {
        Some(crate::midi::Feedback::open()?)
    } else {
        None
    };
    let mut playback = if options.listen {
        Some(Playback::start(&options)?)
    } else {
        None
    };
    let mut terminal = if options.headless {
        None
    } else {
        Some(ratatui::init())
    };
    // ratatui's panic hook restores the terminal. This guard also restores on errors.
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            if self.0 {
                ratatui::restore();
            }
        }
    }
    let _restore = Restore(terminal.is_some());
    let clock = Instant::now();
    let mut tick = 0;
    let mut blackout = false;
    let mut previous_decision = None;
    while !stop.flag.load(Ordering::Relaxed) {
        let Some(mut state) = session.next(options.bursts && !blackout)? else {
            break;
        };
        if state.music.at > end {
            break;
        }
        let due = state.music.at.saturating_sub(start);
        loop {
            let remaining = due.saturating_sub(clock.elapsed());
            if !options.headless && event::poll(remaining.min(Duration::from_millis(10)))? {
                if let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press
                {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            stop.flag.store(true, Ordering::Relaxed)
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            stop.flag.store(true, Ordering::Relaxed)
                        }
                        KeyCode::Char('b') => options.bursts = !options.bursts,
                        KeyCode::Char('x') => blackout = !blackout,
                        _ => {}
                    }
                }
            } else if options.headless && !remaining.is_zero() {
                std::thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            if clock.elapsed() >= due || stop.flag.load(Ordering::Relaxed) {
                break;
            }
        }
        if stop.flag.load(Ordering::Relaxed) {
            break;
        }

        // Inhibit transient requests immediately if b/x changed while pacing.
        if !options.bursts || blackout {
            state.pads = state.base_pads;
            state.show.burst_requested = false;
        }
        if blackout || clock.elapsed() > due + Duration::from_millis(250) {
            state.pads = OFF;
        }
        if tick % 4 == 0
            && let Some(feedback) = &feedback
        {
            feedback.send(state.pads)?;
        }
        if tick % 10 == 0
            && let Some(terminal) = &mut terminal
        {
            terminal.draw(|frame| draw(frame, &state, &options, blackout, session.kicks))?;
        }
        if tick % 100 == 0
            && let Some(playback) = &mut playback
        {
            playback.check()?;
        }
        let decision = (state.show.scene, state.show.burst_requested, state.look);
        if options.headless && (tick % 100 == 0 || previous_decision != Some(decision)) {
            println!(
                "{:6.2}s energy={:.2} kick/s={:.0} scene={:?} burst={} MIDI={} look={}",
                state.music.at.as_secs_f64(),
                state.music.energy,
                state.music.kick_density,
                state.show.scene,
                state.show.burst_requested,
                options.midi,
                state.look
            );
        }
        previous_decision = Some(decision);
        tick += 1;
    }
    // Explicitly clear while still inside the signal/terminal lifetime guards.
    drop(feedback);
    drop(playback);
    Ok(())
}

fn draw(frame: &mut Frame, state: &State, options: &Options, blackout: bool, kicks: u32) {
    if frame.area().width < 80 || frame.area().height < 25 {
        frame.render_widget(
            Paragraph::new("shr-lux needs 80x25. q quits."),
            frame.area(),
        );
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Min(1),
    ])
    .split(frame.area());
    frame.render_widget(
        Paragraph::new(format!(
            "shr-lux | {} | {:.1}s | DMX DISABLED | {}",
            options.program.name(),
            state.music.at.as_secs_f32(),
            if blackout {
                "BLACKOUT"
            } else if options.midi {
                "MiniLab pads"
            } else {
                "terminal pads"
            }
        ))
        .block(Block::bordered()),
        rows[0],
    );
    let m = state.music;
    let mut text = String::new();
    for (i, name) in ["Kick", "Bass", "Guitar 1", "Guitar 2"].iter().enumerate() {
        let db = 20.0 * m.rms[i].max(0.000001).log10();
        text += &format!(
            "{name:8} {db:6.1} dBFS  {:3.0}%  {}\n",
            100.0 * m.normalized[i],
            if m.active[i] { "active" } else { "quiet" }
        );
    }
    frame.render_widget(
        Paragraph::new(text).block(Block::bordered().title("Measured source activity")),
        rows[1],
    );
    let columns = Layout::horizontal([Constraint::Ratio(1, 8); 8]).split(rows[2]);
    for i in 0..8 {
        let rgb = state.pads[i];
        let luminance = u32::from(rgb[0]) * 299 + u32::from(rgb[1]) * 587 + u32::from(rgb[2]) * 114;
        frame.render_widget(
            Paragraph::new(format!(
                "{}\n{}",
                PAD_NAMES[i],
                if rgb == [0; 3] { "off" } else { "on" }
            ))
            .style(Style::default().bg(Color::Rgb(rgb[0], rgb[1], rgb[2])).fg(
                if luminance > 128000 {
                    Color::Black
                } else {
                    Color::White
                },
            ))
            .block(Block::bordered()),
            columns[i],
        );
    }
    frame.render_widget(Paragraph::new(format!(
        "{} | scene hold {}s | fade {}s\nKick: paired accents | Bass: spread and level\nGuitars: left/right motion | burst: three paired flashes",
        state.look, options.program.minimum_scene().as_secs(), options.program.fade().as_secs()
    )).block(Block::bordered().title("Eight-fixture show")),rows[3]);
    let pulse = m
        .pulse
        .map(|p| format!("{:.1} BPM ({:.0}% regular)", p.bpm, p.regularity * 100.0))
        .unwrap_or_else(|| "unavailable".into());
    frame.render_widget(Paragraph::new(format!("Scene {:?} | energy {:.2} / slow {:.2} | kicks {} ({:.0}/s)\nKick pulse: {} | bar / harmony: unavailable\nInput {} | bursts {} | {}",state.show.scene,m.energy,m.slow_energy,kicks,m.kick_density,pulse,if m.reliable {"ready"} else {"settling / unreliable"},if options.bursts {"enabled"} else {"disabled"},state.look)).block(Block::bordered()),rows[4]);
    frame.render_widget(
        Paragraph::new("q/Esc/Ctrl+C quit | b bursts | x blackout | Mouse belongs to terminal"),
        rows[5],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_reject_unknown_and_invalid_times() {
        for args in [
            vec!["--start", "NaN"],
            vec!["--seconds", "0"],
            vec!["--nope"],
            vec!["--file"],
        ] {
            assert!(
                Options::parse(
                    "punk",
                    &args.into_iter().map(String::from).collect::<Vec<_>>()
                )
                .is_err()
            );
        }
        assert!(!Options::parse("metal", &[]).unwrap().midi);
    }
    #[test]
    fn live_screen_fits_and_exposes_output_and_exit_state() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut analyzer =
            crate::analysis::Analyzer::new(crate::analysis::Calibration::new([0.1; 4]).unwrap());
        let music = analyzer.process(0, &[[0.0; 4]; 480]).unwrap();
        let show = crate::show::Director::new(Program::Punk).update(music.at, None, false);
        let state = State {
            music,
            show,
            pads: OFF,
            base_pads: OFF,
            look: "Mirror sweep",
        };
        let options = Options::parse("punk", &[]).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).unwrap();
        terminal
            .draw(|f| draw(f, &state, &options, false, 0))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("DMX DISABLED"));
        assert!(text.contains("Ctrl+C"));
        assert!(text.contains("unavailable"));
    }
}

//! Synthetic features only: no capture, fixtures, USB or DMX.
use shr_lux::show::{Director, Features, Program};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help") {
        println!(
            "Usage: cargo run --locked --example show-preview -- <punk|metal|atmospheric> [--bursts]\nSynthetic 90-second timeline, printed without waiting. OUTPUT DISABLED.\n--bursts permits text-only burst requests; no flash rate is generated."
        );
        return Ok(());
    }
    if args.len() > 2 || (args.len() == 2 && args[1] != "--bursts") {
        return Err("Expected a program and optional --bursts".into());
    }
    let program = Program::ALL
        .into_iter()
        .find(|p| p.name() == args[0])
        .ok_or("Unknown program; use --help")?;
    let mut director = Director::new(program);
    println!("SYNTHETIC PREVIEW | {} | OUTPUT DISABLED", program.name());
    println!("0–5s quiet; 5–30s drive; 30–55s intense/dense kick; 55–60s input loss; 60–90s quiet");
    let mut previous = None;
    for ms in (0..=90_000).step_by(100) {
        let now = Duration::from_millis(ms);
        let (energy, density) = match ms {
            0..5000 => (0.1, 0.0),
            5000..30_000 => (0.6, 3.0),
            30_000..55_000 => (0.9, 12.0),
            _ => (0.1, 0.0),
        };
        let features = (!(55_000..60_000).contains(&ms)).then_some(Features {
            observed_at: now,
            energy,
            kick_hits_per_second: density,
            confidence: 0.95,
        });
        let decision = director.update(now, features, args.len() == 2);
        if previous != Some(decision) {
            println!(
                "{:5.1}s scene={:?} fade={}s burst={} input={}",
                now.as_secs_f32(),
                decision.scene,
                decision.fade.as_secs(),
                decision.burst_requested,
                if decision.input_available {
                    "available"
                } else {
                    "unavailable (hold)"
                }
            );
            previous = Some(decision);
        }
    }
    Ok(())
}

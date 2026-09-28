use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::{
    io::{self, IsTerminal},
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => run_tui()?,
        [arg] if arg == "doctor" => {
            for line in shr_lux::hardware::report()? {
                println!("{line}");
            }
        }
        [arg] if arg == "--help" || arg == "-h" => println!(
            "shr-lux: live-band lighting scaffold\n\nUsage: shr-lux [doctor|--help|--version]\n\nNo arguments: 80x25 TUI (q, Esc, Ctrl+C quit).\ndoctor: read-only uDMX USB diagnostics.\nThis build never sends DMX values."
        ),
        [arg] if arg == "--version" => println!("shr-lux {}", env!("CARGO_PKG_VERSION")),
        _ => return Err("Unknown arguments; use --help".into()),
    }
    Ok(())
}

fn run_tui() -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other(
            "TUI requires a terminal; use doctor or --help",
        ));
    }
    // Ratatui restores the terminal on normal exit, errors, and Rust panics.
    // Do not enable mouse capture, clipboard access, or link interception.
    ratatui::run(|terminal| {
        loop {
            terminal.draw(shr_lux::ui::draw)?;
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && (matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)))
            {
                return Ok(());
            }
        }
    })
}

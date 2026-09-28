use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph, Wrap},
};

pub fn draw(frame: &mut Frame) {
    let area = frame.area();
    if area.width < 80 || area.height < 25 {
        frame.render_widget(
            Paragraph::new("shr-lux needs 80x25. Resize terminal; q quits."),
            area,
        );
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new("shr-lux | Setup shell | OUTPUT DISABLED").block(Block::bordered()),
        rows[0],
    );
    frame.render_widget(Paragraph::new(
        "Kick      unconfigured\nBass      unconfigured\nGuitar 1  unconfigured\nGuitar 2  unconfigured\nTempo / bar / harmony: unavailable")
        .block(Block::bordered().title("Inputs and musical state")), rows[1]);
    frame.render_widget(Paragraph::new(
        "Fixture patch: empty\nDMX transport: not started\nMIDI controller: unconfigured\nManual controls and automation: not implemented\nNo lighting commands are sent by this scaffold.")
        .block(Block::bordered().title("Show")), rows[2]);
    frame.render_widget(Paragraph::new(
        "Read-only hardware check: shr-lux doctor\nProject notebook: docs/index.md\nhttps://github.com/PaolaShultz/shr-lux")
        .wrap(Wrap { trim: false }).block(Block::bordered().title("Reference")), rows[3]);
    frame.render_widget(
        Paragraph::new("q / Esc / Ctrl+C: quit | Mouse and clipboard belong to your terminal"),
        rows[4],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn target_screen_shows_output_state_and_exit_help() {
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).unwrap();
        terminal.draw(draw).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("OUTPUT DISABLED"));
        assert!(text.contains("Ctrl+C: quit"));
        assert!(text.contains("Guitar 2  unconfigured"));
    }

    #[test]
    fn undersized_screen_remains_usable() {
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal.draw(draw).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("needs 80x25"));
    }
}

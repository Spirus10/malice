use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup);
    let lines = vec![
        Line::from("Browse"),
        Line::from("j/k or arrows move"),
        Line::from("Enter bind selected implant and open agent commands"),
        Line::from(": open teamserver commands"),
        Line::from("a open agent commands for selected implant"),
        Line::from("Tab switch between teamserver and agent contexts"),
        Line::from("v view the latest task result in full"),
        Line::from("/ filter implants"),
        Line::from("t open task actions"),
        Line::from("r refresh"),
        Line::from("q quit"),
        Line::from(""),
        Line::from("Command or Filter"),
        Line::from("Enter submit"),
        Line::from("Esc return to browse"),
        Line::from("Up/Down history in the active command context"),
        Line::from("Agent commands: whoami, help, back"),
    ];
    let widget = Paragraph::new(lines)
        .block(Block::default().title("Help").borders(Borders::ALL))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

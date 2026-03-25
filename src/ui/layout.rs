//! Splits the screen into the stable regions used by the dashboard.

use ratatui::layout::{Constraint, Layout, Rect};

use super::state::CommandContextMode;

pub struct ScreenLayout {
    pub header: Rect,
    pub implants: Rect,
    pub details: Rect,
    pub activity: Rect,
    pub command: Rect,
    pub footer: Rect,
}

pub fn split(area: Rect, command_context: CommandContextMode) -> ScreenLayout {
    let vertical = match command_context {
        CommandContextMode::Teamserver => Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Length(7),
            Constraint::Length(3),
        ])
        .split(area),
        CommandContextMode::Agent { .. } => Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Length(3),
        ])
        .split(area),
    };
    let body = Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(vertical[1]);

    ScreenLayout {
        header: vertical[0],
        implants: body[0],
        details: body[1],
        activity: vertical[2],
        command: vertical[3],
        footer: vertical[4],
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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

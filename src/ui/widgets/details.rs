//! Shows details about the selected implant and its latest task.

use std::time::SystemTime;

use ratatui::{
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{core::command::output, ui::state::UiState};

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, state: &UiState) {
    let lines = state
        .selected_implant()
        .map(|record| {
            let mut lines: Vec<Line> = output::format_implant_info(record)
                .into_iter()
                .map(Line::from)
                .collect();
            lines.push(Line::from(""));
            lines.extend(
                output::preview_task_result(state.data.latest_task.as_ref())
                    .into_iter()
                    .map(|line| Line::styled(line, Style::default().fg(Color::Yellow))),
            );
            lines
        })
        .unwrap_or_else(|| vec![Line::from("No implant selected")]);

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Implant Details")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}

pub fn heartbeat_age(timestamp: SystemTime) -> String {
    match SystemTime::now().duration_since(timestamp) {
        Ok(duration) if duration.as_secs() < 60 => format!("{}s", duration.as_secs()),
        Ok(duration) if duration.as_secs() < 3600 => format!("{}m", duration.as_secs() / 60),
        Ok(duration) => format!("{}h", duration.as_secs() / 3600),
        Err(_) => "0s".to_string(),
    }
}

//! Renders recent activity for the whole teamserver or the focused implant.

use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    core::{activity::ActivitySeverity, command::output},
    ui::state::UiState,
};

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, state: &UiState) {
    let sections =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).split(area);

    let preview = Paragraph::new(
        output::preview_task_result(state.data.latest_task.as_ref())
            .into_iter()
            .map(Line::from)
            .collect::<Vec<_>>(),
    )
    .block(Block::default().title("Task Preview").borders(Borders::ALL))
    .wrap(Wrap { trim: true });
    frame.render_widget(preview, sections[0]);

    let events = if state.data.activity.is_empty() {
        vec![Line::from("No recent activity")]
    } else {
        state
            .data
            .activity
            .iter()
            .map(|event| {
                let timestamp = output::format_time(event.timestamp);
                let style = match event.severity {
                    ActivitySeverity::Info => Style::default().fg(Color::Blue),
                    ActivitySeverity::Success => Style::default().fg(Color::Green),
                    ActivitySeverity::Error => Style::default().fg(Color::Red),
                };
                let mut spans = vec![
                    Span::styled(
                        format!("[{}] ", &timestamp[11..]),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(event.message.clone(), style),
                ];
                if let Some(task_id) = event.task_id {
                    spans.push(Span::styled(
                        format!(
                            " ({})",
                            task_id.to_string().chars().take(8).collect::<String>()
                        ),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                Line::from(spans)
            })
            .collect()
    };

    let widget = Paragraph::new(events)
        .block(Block::default().title("Activity").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, sections[1]);
}

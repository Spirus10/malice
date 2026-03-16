//! Renders the modal viewer for the latest task result text.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    core::tasks::{TaskResultData, TaskStatus},
    ui::state::UiState,
};

pub fn render(frame: &mut Frame, area: Rect, state: &UiState) {
    let popup = centered_rect(80, 75, area);
    frame.render_widget(Clear, popup);

    let lines = if let Some(task) = &state.data.latest_task {
        let mut lines = vec![
            Line::from(format!("task_id: {}", task.task_id)),
            Line::from(format!("clientid: {}", task.clientid)),
            Line::from(format!("task_type: {}", task.task_kind)),
            Line::from(format!("status: {:?}", task.status)),
            Line::from(""),
        ];

        match &task.result {
            Some(TaskResultData::Text { data, .. }) => {
                lines.extend(data.lines().map(|line| Line::from(line.to_string())));
            }
            None => {
                lines.push(Line::from(match task.status {
                    TaskStatus::Queued | TaskStatus::Leased | TaskStatus::Acknowledged => {
                        "result pending"
                    }
                    TaskStatus::TimedOut => "task timed out",
                    TaskStatus::Completed | TaskStatus::Failed => "no result body",
                }));
            }
        }

        lines
    } else {
        vec![Line::from("No recent task result available")]
    };

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Latest Result Viewer")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
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

//! Renders the modal viewer for the latest task result text.

use ratatui::{
    layout::Rect,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    core::tasks::{TaskResultData, TaskStatus},
    ui::{layout::centered_rect, state::UiState},
};

pub fn render(frame: &mut Frame, area: Rect, state: &UiState) {
    let popup = centered_rect(80, 75, area);
    frame.render_widget(Clear, popup);
    let inner_height = popup.height.saturating_sub(2);
    let scroll = state.result_viewer_scroll.min(state.result_viewer_max_scroll(inner_height));

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
                .title(format!(
                    "Latest Result Viewer [{} / {}] j/k scroll PgUp/PgDn Home/End close: Esc",
                    scroll.saturating_add(1),
                    state.result_viewer_max_scroll(inner_height).saturating_add(1)
                ))
                .borders(Borders::ALL),
        )
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, popup);
}

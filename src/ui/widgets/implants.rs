//! Renders the implant list and current selection.

use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::ui::state::UiState;

use super::details::heartbeat_age;

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, state: &UiState) {
    let rows: Vec<Row> = state
        .data
        .visible_implants
        .iter()
        .map(|record| {
            let id = record.identity.clientid.to_string();
            let short_id: String = id.chars().take(8).collect();
            Row::new(vec![
                Cell::from(short_id),
                Cell::from(record.static_metadata.hostname.clone()),
                Cell::from(record.static_metadata.username.clone()),
                Cell::from(record.static_metadata.process_name.clone()),
                Cell::from(record.runtime_state.current_status.clone()),
                Cell::from(heartbeat_age(record.runtime_state.last_heartbeat)),
                Cell::from(
                    record
                        .runtime_state
                        .active_task_id
                        .map(|task| task.to_string().chars().take(8).collect::<String>())
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Percentage(18),
            Constraint::Percentage(18),
            Constraint::Percentage(20),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec![
            "client", "host", "user", "process", "status", "heart", "task",
        ])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::default().title("Implants").borders(Borders::ALL))
    .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::Black));

    let mut table_state = TableState::default();
    if !state.data.visible_implants.is_empty() {
        table_state.select(Some(state.selected_index));
    }

    frame.render_stateful_widget(table, area, &mut table_state);
}

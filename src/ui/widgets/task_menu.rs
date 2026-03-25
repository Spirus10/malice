//! Renders the quick task selection overlay.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::{layout::centered_rect, state::UiState};

pub fn render(frame: &mut Frame, area: Rect, state: &UiState) {
    let popup = centered_rect(62, 50, area);
    frame.render_widget(Clear, popup);

    let items = if state.data.task_menu_actions.is_empty() {
        vec![ListItem::new(Line::from("no task actions available"))]
    } else {
        state
            .data
            .task_menu_actions
            .iter()
            .map(|action| ListItem::new(Line::from(action.label.clone())))
            .collect()
    };

    let selected_index = state.task_menu_index.min(items.len().saturating_sub(1));
    let list = List::new(items)
        .block(
            Block::default()
                .title("Task Actions: Enter selects, Esc closes")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black))
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    list_state.select(Some(selected_index));
    frame.render_stateful_widget(list, popup, &mut list_state);
}

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::state::UiState;

pub fn render(frame: &mut Frame, area: Rect, state: &UiState) {
    let popup = centered_rect(45, 35, area);
    frame.render_widget(Clear, popup);

    let items = vec![
        ListItem::new(Line::from("execute_coff / whoami")),
        ListItem::new(Line::from("view latest result")),
    ];

    let list = List::new(items)
        .block(Block::default().title("Task Actions").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black))
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.task_menu_index));
    frame.render_stateful_widget(list, popup, &mut list_state);
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

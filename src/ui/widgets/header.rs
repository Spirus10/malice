use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::ui::state::{CommandContextMode, UiState};

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, state: &UiState) {
    let active = state
        .active_implant()
        .map(|record| {
            record
                .identity
                .clientid
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        })
        .unwrap_or_else(|| "none".to_string());
    let server_style = if state.data.server_running {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };
    let (context_label, context_value) = match state.command_context {
        CommandContextMode::Teamserver => ("context ", "teamserver".to_string()),
        CommandContextMode::Agent { clientid } => (
            "context ",
            format!(
                "agent:{}",
                clientid.to_string().chars().take(8).collect::<String>()
            ),
        ),
    };

    let line = Line::from(vec![
        Span::styled("server ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if state.data.server_running {
                "online"
            } else {
                "offline"
            },
            server_style,
        ),
        Span::raw(" | "),
        Span::styled("addr ", Style::default().fg(Color::DarkGray)),
        Span::raw(state.data.server_addr.clone()),
        Span::raw(" | "),
        Span::styled("active ", Style::default().fg(Color::DarkGray)),
        Span::raw(active),
        Span::raw(" | "),
        Span::styled("filter ", Style::default().fg(Color::DarkGray)),
        Span::raw(if state.filter_input.is_empty() {
            "none".to_string()
        } else {
            state.filter_input.clone()
        }),
        Span::raw(" | "),
        Span::styled(context_label, Style::default().fg(Color::DarkGray)),
        Span::raw(context_value),
        Span::raw(" | "),
        Span::styled("focus ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{:?}", state.mode)),
    ]);

    let widget = Paragraph::new(line).block(Block::default().title("Header").borders(Borders::ALL));
    frame.render_widget(widget, area);
}

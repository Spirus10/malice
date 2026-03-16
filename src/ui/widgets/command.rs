//! Renders the command/output pane for teamserver and agent contexts.

use ratatui::{
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::ui::state::{CommandContextMode, Mode, UiState};

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, state: &UiState) {
    let (title, lines) = match state.command_context {
        CommandContextMode::Teamserver => render_teamserver(state),
        CommandContextMode::Agent { .. } => render_agent(state),
    };

    let widget = Paragraph::new(lines)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}

fn render_teamserver(state: &UiState) -> (String, Vec<Line<'static>>) {
    let prompt = match state.mode {
        Mode::TeamserverCommand => Line::styled(
            format!("teamserver> {}", state.teamserver_input),
            Style::default().fg(Color::Yellow),
        ),
        Mode::Filter => Line::styled(
            format!("filter> {}", state.filter_input),
            Style::default().fg(Color::Cyan),
        ),
        _ => Line::from("> : to enter teamserver command mode"),
    };

    let mut lines = vec![prompt];
    if state.teamserver_output.is_empty() {
        lines.push(Line::from(
            "global commands: httpserver, implants, task queue, task result",
        ));
        let advertised = if state.data.tasking_metadata.command_names.is_empty() {
            "tasks depend on the selected implant".to_string()
        } else {
            format!(
                "tasks for selected implant: {}",
                state.data.tasking_metadata.command_names.join(", ")
            )
        };
        lines.push(Line::from(advertised));
        lines.push(Line::from(
            "examples: task queue selected pwd | task queue selected ls \"C:\\Users\\Public\\*\"",
        ));
        lines.push(Line::from(
            "press `t` for task templates, `:` for raw teamserver commands",
        ));
    } else {
        lines.extend(
            state
                .teamserver_output
                .iter()
                .take(2)
                .cloned()
                .map(Line::from),
        );
    }

    ("Teamserver Commands".to_string(), lines)
}

fn render_agent(state: &UiState) -> (String, Vec<Line<'static>>) {
    let mut lines = Vec::new();
    let Some(record) = state.bound_agent() else {
        lines.push(Line::from("No bound implant"));
        return ("Agent Commands".to_string(), lines);
    };

    let short_id: String = record
        .identity
        .clientid
        .to_string()
        .chars()
        .take(8)
        .collect();
    let supported = state.supported_agent_commands();
    let title = if supported.is_empty() {
        format!("Agent Commands [{}] no supported actions", short_id)
    } else {
        format!("Agent Commands [{}] {}", short_id, supported.join(", "))
    };

    lines.push(Line::styled(
        format!(
            "bound to {} {}\\{}",
            record.identity.clientid,
            record.static_metadata.hostname,
            record.static_metadata.username
        ),
        Style::default().fg(Color::Cyan),
    ));
    lines.push(Line::from(if supported.is_empty() {
        "supported: help, back".to_string()
    } else {
        format!("supported: {}, help, back", supported.join(", "))
    }));
    lines.push(Line::from(""));
    lines.push(match state.mode {
        Mode::AgentCommand => Line::styled(
            format!("agent> {}", state.agent_input),
            Style::default().fg(Color::Yellow),
        ),
        _ => Line::from("Press `a` or Tab to focus agent commands"),
    });
    lines.push(Line::from(""));
    if state.agent_output.is_empty() {
        lines.push(Line::from("No agent command output yet"));
    } else {
        lines.extend(state.agent_output.iter().take(4).cloned().map(Line::from));
    }

    (title, lines)
}

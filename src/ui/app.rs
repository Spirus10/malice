use std::{
    io::{self, Result},
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Frame, Terminal};

use super::{
    actions::UiAction,
    controller::TuiController,
    events, layout,
    state::{Mode, StatusKind, UiState},
    widgets,
};

pub async fn run() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let controller = TuiController::new().await;
    let mut state = UiState::default();
    controller.refresh(&mut state).await?;

    let tick_rate = Duration::from_millis(250);
    let refresh_rate = Duration::from_secs(1);
    let mut last_tick = Instant::now();
    let mut last_refresh = Instant::now();

    loop {
        terminal.draw(|frame| draw(frame, &state))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(action) = events::map_key(state.mode, key) {
                        if let Err(err) = controller.handle_action(action, &mut state).await {
                            state.set_status(StatusKind::Error, err.to_string());
                        }
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            if let Err(err) = controller.handle_action(UiAction::Tick, &mut state).await {
                state.set_status(StatusKind::Error, err.to_string());
            }
            last_tick = Instant::now();
        }

        if last_refresh.elapsed() >= refresh_rate {
            if let Err(err) = controller
                .handle_action(UiAction::Refresh, &mut state)
                .await
            {
                state.set_status(StatusKind::Error, err.to_string());
            }
            last_refresh = Instant::now();
        }

        if state.should_quit {
            break;
        }
    }

    restore_terminal(&mut terminal)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

fn draw(frame: &mut Frame, state: &UiState) {
    let chunks = layout::split(frame.area(), state.command_context);
    widgets::header::render(frame, chunks.header, state);
    widgets::implants::render(frame, chunks.implants, state);
    widgets::details::render(frame, chunks.details, state);
    widgets::activity::render(frame, chunks.activity, state);
    widgets::command::render(frame, chunks.command, state);
    render_footer(frame, chunks.footer, state);

    match state.mode {
        Mode::Help => widgets::help::render(frame, frame.area()),
        Mode::ResultViewer => widgets::result_viewer::render(frame, frame.area(), state),
        Mode::TaskMenu => widgets::task_menu::render(frame, frame.area(), state),
        _ => {}
    }
}

fn render_footer(frame: &mut Frame, area: ratatui::layout::Rect, state: &UiState) {
    let sections = ratatui::layout::Layout::horizontal([
        ratatui::layout::Constraint::Percentage(40),
        ratatui::layout::Constraint::Percentage(60),
    ])
    .split(area);

    let color = match state.status.kind {
        StatusKind::Info => ratatui::style::Color::Cyan,
        StatusKind::Success => ratatui::style::Color::Green,
        StatusKind::Error => ratatui::style::Color::Red,
    };

    let status = ratatui::widgets::Paragraph::new(state.status.message.clone())
        .style(ratatui::style::Style::default().fg(color))
        .block(
            ratatui::widgets::Block::default()
                .title("Status")
                .borders(ratatui::widgets::Borders::ALL),
        );
    frame.render_widget(status, sections[0]);

    let hints = ratatui::widgets::Paragraph::new(
        "j/k move | Enter bind agent | : teamserver | a agent | Tab switch | v result | q quit",
    )
    .style(ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray))
    .block(
        ratatui::widgets::Block::default()
            .title("Keys")
            .borders(ratatui::widgets::Borders::ALL),
    );
    frame.render_widget(hints, sections[1]);
}

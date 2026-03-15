use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{actions::UiAction, state::Mode};

pub fn map_key(mode: Mode, key: KeyEvent) -> Option<UiAction> {
    match mode {
        Mode::Browse => match key.code {
            KeyCode::Char('q') => Some(UiAction::Quit),
            KeyCode::Down | KeyCode::Char('j') => Some(UiAction::SelectNext),
            KeyCode::Up | KeyCode::Char('k') => Some(UiAction::SelectPrevious),
            KeyCode::Enter => Some(UiAction::ActivateSelected),
            KeyCode::Char(':') => Some(UiAction::EnterTeamserverContext),
            KeyCode::Char('a') => Some(UiAction::EnterAgentContext),
            KeyCode::Tab => Some(UiAction::ToggleCommandContext),
            KeyCode::Char('/') => Some(UiAction::FocusFilter),
            KeyCode::Char('r') => Some(UiAction::Refresh),
            KeyCode::Char('v') => Some(UiAction::OpenResultViewer),
            KeyCode::Char('?') => Some(UiAction::OpenHelp),
            KeyCode::Char('t') => Some(UiAction::OpenTaskMenu),
            _ => None,
        },
        Mode::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('?') => Some(UiAction::CloseOverlay),
            _ => None,
        },
        Mode::ResultViewer => match key.code {
            KeyCode::Esc | KeyCode::Char('v') => Some(UiAction::CloseOverlay),
            _ => None,
        },
        Mode::TaskMenu => match key.code {
            KeyCode::Esc => Some(UiAction::CloseOverlay),
            KeyCode::Down | KeyCode::Char('j') => Some(UiAction::TaskMenuNext),
            KeyCode::Up | KeyCode::Char('k') => Some(UiAction::TaskMenuPrevious),
            KeyCode::Enter => Some(UiAction::ConfirmTaskMenu),
            _ => None,
        },
        Mode::TeamserverCommand | Mode::AgentCommand | Mode::Filter => map_text_input(mode, key),
    }
}

fn map_text_input(mode: Mode, key: KeyEvent) -> Option<UiAction> {
    match key.code {
        KeyCode::Tab => Some(UiAction::ToggleCommandContext),
        KeyCode::Esc => Some(UiAction::CloseOverlay),
        KeyCode::Enter => Some(UiAction::SubmitInput),
        KeyCode::Backspace => Some(UiAction::Backspace),
        KeyCode::Up => Some(UiAction::HistoryPrevious),
        KeyCode::Down => Some(UiAction::HistoryNext),
        KeyCode::Char(c)
            if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
        {
            if mode == Mode::Filter && c == '/' {
                None
            } else {
                Some(UiAction::AddChar(c))
            }
        }
        _ => None,
    }
}

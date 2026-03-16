//! High-level actions emitted by the keyboard input layer.

#[derive(Debug, Clone)]
pub enum UiAction {
    Tick,
    Quit,
    SelectNext,
    SelectPrevious,
    ActivateSelected,
    EnterTeamserverContext,
    EnterAgentContext,
    ToggleCommandContext,
    FocusFilter,
    OpenHelp,
    OpenResultViewer,
    CloseOverlay,
    Refresh,
    Backspace,
    SubmitInput,
    AddChar(char),
    HistoryPrevious,
    HistoryNext,
    OpenTaskMenu,
    TaskMenuNext,
    TaskMenuPrevious,
    ConfirmTaskMenu,
}

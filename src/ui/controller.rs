//! Translates UI actions into state transitions and server-side operations.

use std::io::{Error, ErrorKind, Result};

use crate::core::{
    command::{tokenize_command_line, CommandHandler, ImplantCommand, ParsedCommand, TaskCommand},
    implants::ImplantRecord,
    integrations::DEFAULT_RESULT_ACTION_ID,
};

use super::{
    actions::UiAction,
    state::{CommandContextMode, Mode, StatusKind, UiState},
};

pub struct TuiController {
    handler: CommandHandler,
}

impl TuiController {
    /// Builds a controller with a fully initialized command handler.
    ///
    /// @return Controller instance ready to refresh state and handle actions.
    pub async fn new() -> Self {
        Self {
            handler: CommandHandler::new().await,
        }
    }

    /// Refreshes the screen model from the current teamserver state.
    ///
    /// @param state Mutable UI state to repopulate from server-side data.
    /// @return I/O result describing whether the refresh succeeded.
    pub async fn refresh(&self, state: &mut UiState) -> Result<()> {
        let context = self.handler.context();
        let server = context.server_context().await;
        state.data.server_running = context.server_running().await;
        state.data.server_addr = context.server_addr().await;
        state.data.implants = server.list_implants().await;
        state.apply_filter();

        if state.active_clientid.is_none() {
            state.activate_selected();
        }

        if let CommandContextMode::Agent { clientid } = state.command_context {
            let still_present = state
                .data
                .implants
                .iter()
                .any(|record| record.identity.clientid == clientid);
            if !still_present {
                state.command_context = CommandContextMode::Teamserver;
                state.mode = Mode::Browse;
                state.set_status(StatusKind::Error, "bound agent no longer present");
            }
        }

        let focus_clientid = match state.command_context {
            CommandContextMode::Agent { clientid } => Some(clientid),
            CommandContextMode::Teamserver => state
                .selected_implant()
                .map(|record| record.identity.clientid),
        };

        if let Some(clientid) = focus_clientid {
            state.data.activity = server.recent_activity_for_implant(&clientid, 8).await;
            state.data.latest_task = server
                .recent_tasks_for_implant(&clientid, 1)
                .await
                .into_iter()
                .next();
        } else {
            state.data.activity = server.recent_activity(8).await;
            state.data.latest_task = server.recent_tasks(1).await.into_iter().next();
        }

        let metadata_record = state
            .bound_agent()
            .cloned()
            .or_else(|| state.selected_implant().cloned());
        if let Some(record) = metadata_record.as_ref() {
            state.data.tasking_metadata = server.tasking_metadata(record).unwrap_or_default();
            state.data.task_menu_actions =
                server.ui_actions_for_implant(record).unwrap_or_default();
        } else {
            state.data.tasking_metadata = Default::default();
            state.data.task_menu_actions.clear();
        }

        Ok(())
    }

    /// Applies one normalized UI action to the current state.
    ///
    /// @param action Normalized operator intent emitted by the input layer.
    /// @param state Mutable UI state to update.
    /// @return I/O result describing whether the action completed successfully.
    pub async fn handle_action(&self, action: UiAction, state: &mut UiState) -> Result<()> {
        match action {
            UiAction::Tick | UiAction::Refresh => self.refresh(state).await,
            UiAction::Quit => {
                state.should_quit = true;
                Ok(())
            }
            UiAction::SelectNext => {
                state.select_next();
                self.refresh(state).await
            }
            UiAction::SelectPrevious => {
                state.select_previous();
                self.refresh(state).await
            }
            UiAction::ActivateSelected => {
                if state.enter_agent_context() {
                    if let Some(record) = state.bound_agent() {
                        state.set_status(
                            StatusKind::Success,
                            format!("agent context bound to {}", record.identity.clientid),
                        );
                    }
                    self.refresh(state).await
                } else {
                    Err(Error::new(
                        ErrorKind::NotFound,
                        "No implant selected to bind",
                    ))
                }
            }
            UiAction::EnterTeamserverContext => {
                state.enter_teamserver_context();
                state.set_status(StatusKind::Info, "teamserver command mode");
                Ok(())
            }
            UiAction::EnterAgentContext => {
                if state.enter_agent_context() {
                    if let Some(record) = state.bound_agent() {
                        state.set_status(
                            StatusKind::Success,
                            format!("agent command mode for {}", record.identity.clientid),
                        );
                    }
                    self.refresh(state).await
                } else {
                    Err(Error::new(
                        ErrorKind::NotFound,
                        "No implant selected to bind",
                    ))
                }
            }
            UiAction::ToggleCommandContext => self.toggle_command_context(state).await,
            UiAction::FocusFilter => {
                state.mode = Mode::Filter;
                state.set_status(StatusKind::Info, "filter mode");
                Ok(())
            }
            UiAction::OpenHelp => {
                state.mode = Mode::Help;
                Ok(())
            }
            UiAction::OpenResultViewer => {
                if state.data.latest_task.is_some() {
                    state.result_viewer_scroll = 0;
                    state.mode = Mode::ResultViewer;
                    state.set_status(StatusKind::Info, "latest result viewer");
                    Ok(())
                } else {
                    Err(Error::new(
                        ErrorKind::NotFound,
                        "No recent task result available",
                    ))
                }
            }
            UiAction::CloseOverlay => {
                state.mode = Mode::Browse;
                state.teamserver_history_index = None;
                state.agent_history_index = None;
                Ok(())
            }
            UiAction::ResultScrollUp => {
                state.result_viewer_scroll = state.result_viewer_scroll.saturating_sub(1);
                Ok(())
            }
            UiAction::ResultScrollDown => {
                state.result_viewer_scroll = state.result_viewer_scroll.saturating_add(1);
                Ok(())
            }
            UiAction::ResultPageUp => {
                state.result_viewer_scroll = state.result_viewer_scroll.saturating_sub(10);
                Ok(())
            }
            UiAction::ResultPageDown => {
                state.result_viewer_scroll = state.result_viewer_scroll.saturating_add(10);
                Ok(())
            }
            UiAction::ResultScrollTop => {
                state.result_viewer_scroll = 0;
                Ok(())
            }
            UiAction::ResultScrollBottom => {
                state.result_viewer_scroll = u16::MAX;
                Ok(())
            }
            UiAction::Backspace => {
                match state.mode {
                    Mode::TeamserverCommand | Mode::AgentCommand => {
                        if let Some(input) = state.current_input_mut() {
                            input.pop();
                        }
                    }
                    Mode::Filter => {
                        state.filter_input.pop();
                        state.apply_filter();
                    }
                    _ => {}
                }
                Ok(())
            }
            UiAction::SubmitInput => match state.mode {
                Mode::TeamserverCommand => self.submit_teamserver_command(state).await,
                Mode::AgentCommand => self.submit_agent_command(state).await,
                Mode::Filter => {
                    state.mode = Mode::Browse;
                    self.refresh(state).await
                }
                _ => Ok(()),
            },
            UiAction::AddChar(c) => {
                match state.mode {
                    Mode::TeamserverCommand | Mode::AgentCommand => {
                        if let Some(input) = state.current_input_mut() {
                            input.push(c);
                        }
                    }
                    Mode::Filter => {
                        state.filter_input.push(c);
                        state.apply_filter();
                    }
                    _ => {}
                }
                Ok(())
            }
            UiAction::HistoryPrevious => {
                match state.mode {
                    Mode::TeamserverCommand => self.move_teamserver_history(state, -1),
                    Mode::AgentCommand => self.move_agent_history(state, -1),
                    _ => {}
                }
                Ok(())
            }
            UiAction::HistoryNext => {
                match state.mode {
                    Mode::TeamserverCommand => self.move_teamserver_history(state, 1),
                    Mode::AgentCommand => self.move_agent_history(state, 1),
                    _ => {}
                }
                Ok(())
            }
            UiAction::OpenTaskMenu => {
                state.mode = Mode::TaskMenu;
                state.task_menu_index = 0;
                Ok(())
            }
            UiAction::TaskMenuNext => {
                state.task_menu_index = (state.task_menu_index + 1)
                    .min(state.data.task_menu_actions.len().saturating_sub(1));
                Ok(())
            }
            UiAction::TaskMenuPrevious => {
                state.task_menu_index = state.task_menu_index.saturating_sub(1);
                Ok(())
            }
            UiAction::ConfirmTaskMenu => self.confirm_task_menu(state).await,
        }
    }

    /// Toggles between teamserver and agent command contexts.
    ///
    /// @param state Mutable UI state to update.
    /// @return I/O result describing whether the context switch succeeded.
    async fn toggle_command_context(&self, state: &mut UiState) -> Result<()> {
        match state.command_context {
            CommandContextMode::Teamserver => {
                if state.enter_agent_context() {
                    if let Some(record) = state.bound_agent() {
                        state.set_status(
                            StatusKind::Success,
                            format!("agent command mode for {}", record.identity.clientid),
                        );
                    }
                    self.refresh(state).await
                } else {
                    Err(Error::new(
                        ErrorKind::NotFound,
                        "No implant selected to bind",
                    ))
                }
            }
            CommandContextMode::Agent { .. } => {
                state.enter_teamserver_context();
                state.set_status(StatusKind::Info, "teamserver command mode");
                Ok(())
            }
        }
    }

    /// Moves the teamserver command-history cursor and updates the input buffer.
    ///
    /// @param state Mutable UI state containing the history buffer.
    /// @param delta Signed offset applied to the current history cursor.
    /// @return None.
    fn move_teamserver_history(&self, state: &mut UiState, delta: isize) {
        if state.teamserver_history.is_empty() {
            return;
        }

        let next = match state.teamserver_history_index {
            Some(index) => index
                .saturating_add_signed(delta)
                .min(state.teamserver_history.len() - 1),
            None if delta < 0 => state.teamserver_history.len() - 1,
            None => 0,
        };

        state.teamserver_history_index = Some(next);
        state.teamserver_input = state.teamserver_history[next].clone();
    }

    /// Moves the agent command-history cursor and updates the input buffer.
    ///
    /// @param state Mutable UI state containing the history buffer.
    /// @param delta Signed offset applied to the current history cursor.
    /// @return None.
    fn move_agent_history(&self, state: &mut UiState, delta: isize) {
        if state.agent_history.is_empty() {
            return;
        }

        let next = match state.agent_history_index {
            Some(index) => index
                .saturating_add_signed(delta)
                .min(state.agent_history.len() - 1),
            None if delta < 0 => state.agent_history.len() - 1,
            None => 0,
        };

        state.agent_history_index = Some(next);
        state.agent_input = state.agent_history[next].clone();
    }

    /// Executes the currently selected task-menu action.
    ///
    /// @param state Mutable UI state containing the selected action and bound agent.
    /// @return I/O result describing whether the task action completed successfully.
    async fn confirm_task_menu(&self, state: &mut UiState) -> Result<()> {
        let Some(action) = state
            .data
            .task_menu_actions
            .get(state.task_menu_index)
            .cloned()
        else {
            return Ok(());
        };

        if action.id == DEFAULT_RESULT_ACTION_ID {
            if let Some(task) = &state.data.latest_task {
                state.teamserver_input = format!("task result {}", task.task_id);
                state.enter_teamserver_context();
                state.set_status(StatusKind::Info, "latest task command inserted");
            } else {
                state.mode = Mode::Browse;
                state.set_status(StatusKind::Error, "no recent task for selection");
            }
            return Ok(());
        }

        if action.queue_immediately {
            let Some(task_kind) = action.task_kind else {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "task action missing task kind",
                ));
            };
            let record = self.bound_agent_record(state)?;
            let task = self
                .handler
                .context()
                .queue_task(
                    record.identity.clientid,
                    &task_kind,
                    &action
                        .args_template
                        .iter()
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>(),
                )
                .await
                .map_err(|err| Error::other(err.to_string()))?;

            state.agent_output = vec![
                format!("queued {} for {}", task_kind, task.clientid),
                format!("task_id {}", task.task_id),
            ];
            state.agent_input.clear();
            state.mode = Mode::Browse;
            state.set_status(StatusKind::Success, format!("queued {task_kind}"));
            self.refresh(state).await
        } else if let Some(template) = action.command_template {
            self.insert_task_template(state, &template);
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::InvalidInput,
                "task action missing command template",
            ))
        }
    }

    /// Parses and executes the current teamserver command buffer.
    ///
    /// @param state Mutable UI state containing the teamserver input and output buffers.
    /// @return I/O result describing whether command execution succeeded.
    async fn submit_teamserver_command(&self, state: &mut UiState) -> Result<()> {
        let input = state.teamserver_input.trim().to_string();
        if input.is_empty() {
            state.mode = Mode::Browse;
            return Ok(());
        }

        let parsed = CommandHandler::parse_command(&input)
            .map_err(|err| Error::new(ErrorKind::InvalidInput, err.to_string()))?;
        let parsed = self.resolve_aliases(parsed, state)?;
        let output = match self.handler.handle(parsed).await {
            Ok(output) => output,
            Err(err) if err.is_exit() => {
                state.should_quit = true;
                return Ok(());
            }
            Err(err) => return Err(Error::other(err.to_string())),
        };

        state.teamserver_output = output.as_lines().to_vec();
        state.push_teamserver_history(input);
        state.teamserver_input.clear();
        state.mode = Mode::Browse;
        state.set_status(StatusKind::Success, "teamserver command executed");
        self.refresh(state).await
    }

    /// Parses and executes the current agent command buffer.
    ///
    /// @param state Mutable UI state containing the agent input and output buffers.
    /// @return I/O result describing whether command execution or queueing succeeded.
    async fn submit_agent_command(&self, state: &mut UiState) -> Result<()> {
        let input = state.agent_input.trim().to_string();
        if input.is_empty() {
            state.mode = Mode::Browse;
            return Ok(());
        }

        let tokens = tokenize_command_line(&input)
            .map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;
        let Some((command, args)) = tokens.split_first() else {
            state.mode = Mode::Browse;
            return Ok(());
        };
        let command = command.to_ascii_lowercase();

        match command.as_str() {
            "help" => {
                state.agent_output = self.agent_help_lines(state);
                state.push_agent_history(input);
                state.agent_input.clear();
                state.mode = Mode::Browse;
                state.set_status(StatusKind::Info, "agent help");
                Ok(())
            }
            "back" => {
                state.push_agent_history(input);
                state.agent_input.clear();
                state.enter_teamserver_context();
                state.set_status(StatusKind::Info, "returned to teamserver command mode");
                Ok(())
            }
            _ if state
                .supported_agent_commands()
                .iter()
                .any(|candidate| candidate == &command) =>
            {
                let record = self.bound_agent_record(state)?;
                let task = self
                    .handler
                    .context()
                    .queue_task(record.identity.clientid, &command, args)
                    .await
                    .map_err(|err| Error::other(err.to_string()))?;

                state.agent_output = vec![
                    format!("queued {} for {}", command, task.clientid),
                    format!("task_id {}", task.task_id),
                ];
                state.push_agent_history(input);
                state.agent_input.clear();
                state.mode = Mode::Browse;
                state.set_status(StatusKind::Success, format!("queued {command}"));
                self.refresh(state).await
            }
            _ => {
                let supported = state.supported_agent_commands();
                state.set_status(
                    StatusKind::Error,
                    if supported.is_empty() {
                        "no agent commands available for this implant"
                    } else {
                        "unsupported agent command"
                    },
                );
                Err(Error::new(
                    ErrorKind::InvalidInput,
                    "unsupported agent command",
                ))
            }
        }
    }

    /// Resolves operator aliases like `selected` or `active` inside parsed commands.
    ///
    /// @param command Parsed command that may contain aliases.
    /// @param state Current UI state used to resolve implant context.
    /// @return Parsed command with aliases replaced by concrete identifiers.
    fn resolve_aliases(&self, command: ParsedCommand, state: &UiState) -> Result<ParsedCommand> {
        match command {
            ParsedCommand::Tasks(TaskCommand::Queue {
                clientid,
                task_kind,
                args,
            }) => Ok(ParsedCommand::Tasks(TaskCommand::Queue {
                clientid: self.resolve_target(&clientid, state)?,
                task_kind,
                args,
            })),
            ParsedCommand::Implants(ImplantCommand::Info { clientid }) => {
                Ok(ParsedCommand::Implants(ImplantCommand::Info {
                    clientid: self.resolve_target(&clientid, state)?,
                }))
            }
            other => Ok(other),
        }
    }

    /// Resolves one implant target token into a concrete client identifier.
    ///
    /// @param token Target token supplied by the operator.
    /// @param state Current UI state used to resolve aliases.
    /// @return Concrete client identifier string.
    fn resolve_target(&self, token: &str, state: &UiState) -> Result<String> {
        if token != "active" && token != "selected" {
            return Ok(token.to_string());
        }

        state
            .active_implant()
            .map(|record| record.identity.clientid.to_string())
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "No active implant selected"))
    }

    /// Returns the implant currently bound to the agent command context.
    ///
    /// @param state UI state containing the bound agent context.
    /// @return Bound implant record reference.
    fn bound_agent_record<'a>(&self, state: &'a UiState) -> Result<&'a ImplantRecord> {
        state
            .bound_agent()
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "No bound implant selected"))
    }

    /// Builds the help lines shown for agent command mode.
    ///
    /// @param state Current UI state containing task metadata for the bound implant.
    /// @return Help lines describing supported agent commands.
    fn agent_help_lines(&self, state: &UiState) -> Vec<String> {
        let mut lines = vec!["supported agent commands:".to_string()];
        if state.bound_agent().is_none() {
            lines.push("none".to_string());
            lines.push("help".to_string());
            lines.push("back".to_string());
            return lines;
        }

        let supported = state.data.tasking_metadata.command_help.clone();
        if supported.is_empty() {
            lines.push("none".to_string());
        } else {
            lines.extend(supported);
        }

        lines.push("help".to_string());
        lines.push("back".to_string());
        lines
    }

    /// Inserts a task command template into the teamserver input buffer.
    ///
    /// @param state Mutable UI state to update.
    /// @param template Command template text to insert.
    /// @return None.
    fn insert_task_template(&self, state: &mut UiState, template: &str) {
        state.teamserver_input = template.to_string();
        state.enter_teamserver_context();
        state.set_status(StatusKind::Info, "task template inserted");
    }
}

//! Mutable UI state, including selection, overlays, and command buffers.

use crate::core::{
    activity::{ActivityEvent, ActivitySeverity},
    implants::{ImplantRecord, TaskingMetadata},
    integrations::UiActionDefinition,
    tasks::TaskRecord,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Browse,
    TeamserverCommand,
    AgentCommand,
    Filter,
    Help,
    ResultViewer,
    TaskMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandContextMode {
    Teamserver,
    Agent { clientid: uuid::Uuid },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Error,
}

impl From<ActivitySeverity> for StatusKind {
    fn from(value: ActivitySeverity) -> Self {
        match value {
            ActivitySeverity::Success => Self::Success,
            ActivitySeverity::Error => Self::Error,
            ActivitySeverity::Info => Self::Info,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatusLine {
    pub kind: StatusKind,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct UiData {
    pub implants: Vec<ImplantRecord>,
    pub visible_implants: Vec<ImplantRecord>,
    pub activity: Vec<ActivityEvent>,
    pub latest_task: Option<TaskRecord>,
    pub server_running: bool,
    pub server_addr: String,
    pub tasking_metadata: TaskingMetadata,
    pub task_menu_actions: Vec<UiActionDefinition>,
}

#[derive(Debug, Clone)]
pub struct UiState {
    pub mode: Mode,
    pub command_context: CommandContextMode,
    pub selected_index: usize,
    pub active_clientid: Option<uuid::Uuid>,
    pub filter_input: String,
    pub teamserver_input: String,
    pub teamserver_history: Vec<String>,
    pub teamserver_history_index: Option<usize>,
    pub agent_input: String,
    pub agent_history: Vec<String>,
    pub agent_history_index: Option<usize>,
    pub should_quit: bool,
    pub status: StatusLine,
    pub data: UiData,
    pub task_menu_index: usize,
    pub teamserver_output: Vec<String>,
    pub agent_output: Vec<String>,
    pub result_viewer_scroll: u16,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            mode: Mode::Browse,
            command_context: CommandContextMode::Teamserver,
            selected_index: 0,
            active_clientid: None,
            filter_input: String::new(),
            teamserver_input: String::new(),
            teamserver_history: Vec::new(),
            teamserver_history_index: None,
            agent_input: String::new(),
            agent_history: Vec::new(),
            agent_history_index: None,
            should_quit: false,
            status: StatusLine {
                kind: StatusKind::Info,
                message: "ready".to_string(),
            },
            data: UiData::default(),
            task_menu_index: 0,
            teamserver_output: Vec::new(),
            agent_output: Vec::new(),
            result_viewer_scroll: 0,
        }
    }
}

impl UiState {
    pub fn apply_filter(&mut self) {
        let filter = self.filter_input.to_ascii_lowercase();
        self.data.visible_implants = self
            .data
            .implants
            .iter()
            .filter(|record| {
                if filter.is_empty() {
                    return true;
                }

                let clientid = record.identity.clientid.to_string();
                let short_id: String = clientid.chars().take(8).collect();
                let haystack = format!(
                    "{} {} {} {} {}",
                    short_id,
                    record.static_metadata.hostname,
                    record.static_metadata.username,
                    record.static_metadata.process_name,
                    record.runtime_state.current_status
                )
                .to_ascii_lowercase();
                haystack.contains(&filter)
            })
            .cloned()
            .collect();
        self.clamp_selection();
    }

    pub fn clamp_selection(&mut self) {
        if self.data.visible_implants.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.data.visible_implants.len() {
            self.selected_index = self.data.visible_implants.len().saturating_sub(1);
        }
    }

    pub fn selected_implant(&self) -> Option<&ImplantRecord> {
        self.data.visible_implants.get(self.selected_index)
    }

    pub fn active_implant(&self) -> Option<&ImplantRecord> {
        self.active_clientid
            .and_then(|clientid| {
                self.data
                    .implants
                    .iter()
                    .find(|record| record.identity.clientid == clientid)
            })
            .or_else(|| self.selected_implant())
    }

    pub fn bound_agent(&self) -> Option<&ImplantRecord> {
        match self.command_context {
            CommandContextMode::Teamserver => None,
            CommandContextMode::Agent { clientid } => self
                .data
                .implants
                .iter()
                .find(|record| record.identity.clientid == clientid),
        }
    }

    pub fn select_next(&mut self) {
        if !self.data.visible_implants.is_empty() {
            self.selected_index =
                (self.selected_index + 1).min(self.data.visible_implants.len() - 1);
        }
    }

    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn activate_selected(&mut self) {
        self.active_clientid = self
            .selected_implant()
            .map(|record| record.identity.clientid);
    }

    pub fn bind_selected_agent(&mut self) -> bool {
        let Some(clientid) = self
            .selected_implant()
            .map(|record| record.identity.clientid)
        else {
            return false;
        };

        self.active_clientid = Some(clientid);
        self.command_context = CommandContextMode::Agent { clientid };
        true
    }

    pub fn enter_teamserver_context(&mut self) {
        self.command_context = CommandContextMode::Teamserver;
        self.mode = Mode::TeamserverCommand;
        self.teamserver_history_index = None;
    }

    pub fn enter_agent_context(&mut self) -> bool {
        if self.bind_selected_agent() {
            self.mode = Mode::AgentCommand;
            self.agent_history_index = None;
            return true;
        }

        false
    }

    pub fn current_input_mut(&mut self) -> Option<&mut String> {
        match self.mode {
            Mode::TeamserverCommand => Some(&mut self.teamserver_input),
            Mode::AgentCommand => Some(&mut self.agent_input),
            _ => None,
        }
    }

    pub fn supported_agent_commands(&self) -> Vec<String> {
        self.data.tasking_metadata.command_names.clone()
    }

    pub fn set_status(&mut self, kind: StatusKind, message: impl Into<String>) {
        self.status = StatusLine {
            kind,
            message: message.into(),
        };
    }

    pub fn push_teamserver_history(&mut self, value: String) {
        if !value.is_empty() {
            self.teamserver_history.push(value);
        }
        self.teamserver_history_index = None;
    }

    pub fn push_agent_history(&mut self, value: String) {
        if !value.is_empty() {
            self.agent_history.push(value);
        }
        self.agent_history_index = None;
    }

    pub fn result_viewer_line_count(&self) -> usize {
        let Some(task) = &self.data.latest_task else {
            return 1;
        };

        let mut count = 5;
        match &task.result {
            Some(crate::core::tasks::TaskResultData::Text { data, .. }) => {
                count += data.lines().count();
            }
            None => {
                count += 1;
            }
        }

        count
    }

    pub fn result_viewer_max_scroll(&self, viewport_height: u16) -> u16 {
        let visible = usize::from(viewport_height.max(1));
        self.result_viewer_line_count()
            .saturating_sub(visible)
            .try_into()
            .unwrap_or(u16::MAX)
    }

}

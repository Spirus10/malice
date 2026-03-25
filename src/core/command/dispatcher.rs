//! Command dispatcher and shared helper types for operator commands.

use std::path::Path;
use std::{collections::HashMap, error::Error, fmt, future::Future, pin::Pin, process, sync::Arc};

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::core::{app::ServerContext, httpserver::HttpServer};

use super::{
    commands, output,
    parser::{self, ParsedCommand},
};

#[derive(Debug)]
pub struct CommandError {
    details: String,
}

impl CommandError {
    pub fn new(msg: impl Into<String>) -> CommandError {
        CommandError {
            details: msg.into(),
        }
    }
}

impl Error for CommandError {}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

#[derive(Clone)]
pub struct CommandContext {
    server: Arc<Mutex<HttpServer>>,
}

#[derive(Debug, Clone, Default)]
pub struct CommandOutput {
    lines: Vec<String>,
}

impl CommandOutput {
    pub fn line(line: impl Into<String>) -> Self {
        Self {
            lines: vec![line.into()],
        }
    }

    pub fn lines(lines: Vec<String>) -> Self {
        Self { lines }
    }

    pub fn as_lines(&self) -> &[String] {
        &self.lines
    }
}

impl CommandContext {
    /// Starts the HTTP server and records the corresponding activity event.
    ///
    /// @return Renderable command output or a command error if startup fails.
    pub async fn start_server(&self) -> Result<CommandOutput, CommandError> {
        let mut server = self.server.lock().await;
        server
            .start()
            .await
            .map_err(|err| CommandError::new(err.to_string()))?;
        let addr = server.local_addr();
        server
            .context()
            .record_activity(
                crate::core::activity::ActivitySeverity::Success,
                format!("server started on {}", addr),
                None,
                None,
            )
            .await;
        Ok(CommandOutput::line(format!(
            "HTTP server started on {}",
            addr
        )))
    }

    /// Stops the HTTP server and records the corresponding activity event.
    ///
    /// @return Renderable command output confirming shutdown.
    pub async fn stop_server(&self) -> CommandOutput {
        let mut server = self.server.lock().await;
        server.close().await;
        server
            .context()
            .record_activity(
                crate::core::activity::ActivitySeverity::Info,
                "server stopped",
                None,
                None,
            )
            .await;
        CommandOutput::line("HTTP server stopped")
    }

    pub async fn list_implants(&self) -> Vec<crate::core::implants::ImplantRecord> {
        self.server.lock().await.context().list_implants().await
    }

    pub async fn implant_info(
        &self,
        clientid: &Uuid,
    ) -> Option<crate::core::implants::ImplantRecord> {
        self.server
            .lock()
            .await
            .context()
            .implant_info(clientid)
            .await
    }

    /// Queues one operator-requested task for a target implant.
    ///
    /// @param clientid Implant identifier that should receive the task.
    /// @param task_kind User-facing task name to queue.
    /// @param args Additional task arguments supplied by the operator.
    /// @return Queued task record or a command error if queueing fails.
    pub async fn queue_task(
        &self,
        clientid: Uuid,
        task_kind: &str,
        args: &[String],
    ) -> Result<crate::core::tasks::TaskRecord, CommandError> {
        self.server
            .lock()
            .await
            .context()
            .queue_named_task(clientid, task_kind, args)
            .await
            .map_err(|err| CommandError::new(err.to_string()))
    }

    pub async fn task_result(&self, taskid: &Uuid) -> Option<crate::core::tasks::TaskRecord> {
        self.server.lock().await.context().task_result(taskid).await
    }

    pub async fn list_plugins(
        &self,
    ) -> Result<Vec<crate::core::integrations::InstalledPluginSummary>, CommandError> {
        self.server
            .lock()
            .await
            .context()
            .list_plugins()
            .map_err(|err| CommandError::new(err.to_string()))
    }

    pub async fn inspect_plugin(
        &self,
        plugin_id: &str,
        version: Option<&str>,
    ) -> Result<crate::core::integrations::PluginInspection, CommandError> {
        self.server
            .lock()
            .await
            .context()
            .inspect_plugin(plugin_id, version)
            .map_err(|err| CommandError::new(err.to_string()))
    }

    pub async fn install_plugin(
        &self,
        source: &str,
    ) -> Result<crate::core::integrations::InstalledPluginSummary, CommandError> {
        self.server
            .lock()
            .await
            .context()
            .install_plugin(Path::new(source))
            .map_err(|err| CommandError::new(err.to_string()))
    }

    pub async fn activate_plugin(
        &self,
        plugin_id: &str,
        version: Option<&str>,
    ) -> Result<crate::core::integrations::InstalledPluginSummary, CommandError> {
        self.server
            .lock()
            .await
            .context()
            .activate_plugin(plugin_id, version)
            .map_err(|err| CommandError::new(err.to_string()))
    }

    pub async fn deactivate_plugin(&self, plugin_id: &str) -> Result<(), CommandError> {
        self.server
            .lock()
            .await
            .context()
            .deactivate_plugin(plugin_id)
            .map_err(|err| CommandError::new(err.to_string()))
    }

    pub async fn remove_plugin(&self, plugin_id: &str, version: &str) -> Result<(), CommandError> {
        self.server
            .lock()
            .await
            .context()
            .remove_plugin(plugin_id, version)
            .map_err(|err| CommandError::new(err.to_string()))
    }

    pub async fn plugin_load_errors(&self) -> Vec<String> {
        self.server.lock().await.context().plugin_load_errors()
    }

    pub async fn server_running(&self) -> bool {
        self.server.lock().await.is_running()
    }

    pub async fn server_addr(&self) -> String {
        self.server.lock().await.local_addr().to_string()
    }

    pub async fn server_context(&self) -> Arc<ServerContext> {
        self.server.lock().await.context()
    }
}

type CommandFuture = Pin<Box<dyn Future<Output = Result<CommandOutput, CommandError>> + Send>>;
pub type CommandExecutor = fn(Arc<CommandContext>, ParsedCommand) -> CommandFuture;

pub struct CommandHandler {
    context: Arc<CommandContext>,
    handlers: HashMap<&'static str, CommandExecutor>,
}

impl CommandHandler {
    /// Creates a handler backed by a fresh server context and HTTP server.
    ///
    /// @return Command handler ready to parse and execute operator commands.
    pub async fn new() -> Self {
        let server = HttpServer::new(ServerContext::new());
        Self {
            context: Arc::new(CommandContext {
                server: Arc::new(Mutex::new(server)),
            }),
            handlers: commands::registry(),
        }
    }

    /// Parses raw command text into the internal command model.
    ///
    /// @param cmd Raw command line entered by the operator.
    /// @return Parsed command or a command error if validation fails.
    pub fn parse_command(cmd: &str) -> Result<ParsedCommand, CommandError> {
        parser::parse_command(cmd).map_err(CommandError::new)
    }

    /// Executes a parsed command and returns its rendered output.
    ///
    /// @param command Parsed command ready for dispatch.
    /// @return Renderable command output or a command error if execution fails.
    pub async fn handle(&self, command: ParsedCommand) -> Result<CommandOutput, CommandError> {
        match command {
            ParsedCommand::Exit => process::exit(0),
            ParsedCommand::Server(_) => self.execute("server", command).await,
            ParsedCommand::Implants(_) => self.execute("implants", command).await,
            ParsedCommand::Plugins(_) => self.execute("plugins", command).await,
            ParsedCommand::Tasks(_) => self.execute("tasks", command).await,
        }
    }

    async fn execute(
        &self,
        key: &str,
        command: ParsedCommand,
    ) -> Result<CommandOutput, CommandError> {
        let Some(handler) = self.handlers.get(key) else {
            return Err(CommandError::new("No command handler registered"));
        };

        handler(self.context.clone(), command).await
    }

    /// Returns the shared command context for direct callers like the TUI.
    ///
    /// @return Shared command context exposing server-backed operations.
    pub fn context(&self) -> Arc<CommandContext> {
        self.context.clone()
    }
}

pub fn parse_uuid(input: &str) -> Result<Uuid, CommandError> {
    input
        .parse::<Uuid>()
        .map_err(|_| CommandError::new("Invalid UUID"))
}

pub fn show_implant_list(records: &[crate::core::implants::ImplantRecord]) -> CommandOutput {
    CommandOutput::lines(records.iter().map(output::format_implant_list).collect())
}

pub fn show_implant_info(record: &crate::core::implants::ImplantRecord) -> CommandOutput {
    CommandOutput::lines(output::format_implant_info(record))
}

pub fn show_task_result(record: &crate::core::tasks::TaskRecord) -> CommandOutput {
    CommandOutput::lines(output::format_task_result(record))
}

pub fn info(message: &str) -> CommandOutput {
    CommandOutput::line(message)
}

pub fn good(message: &str) -> CommandOutput {
    CommandOutput::line(message)
}

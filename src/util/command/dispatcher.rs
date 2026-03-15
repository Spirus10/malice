use std::{collections::HashMap, error::Error, fmt, future::Future, pin::Pin, process, sync::Arc};

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::util::{app::ServerContext, httpserver::HttpServer};

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
                crate::util::activity::ActivitySeverity::Success,
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

    pub async fn stop_server(&self) -> CommandOutput {
        let mut server = self.server.lock().await;
        server.close().await;
        server
            .context()
            .record_activity(
                crate::util::activity::ActivitySeverity::Info,
                "server stopped",
                None,
                None,
            )
            .await;
        CommandOutput::line("HTTP server stopped")
    }

    pub async fn list_implants(&self) -> Vec<crate::util::implants::ImplantRecord> {
        self.server.lock().await.context().list_implants().await
    }

    pub async fn implant_info(
        &self,
        clientid: &Uuid,
    ) -> Option<crate::util::implants::ImplantRecord> {
        self.server
            .lock()
            .await
            .context()
            .implant_info(clientid)
            .await
    }

    pub async fn queue_task(
        &self,
        clientid: Uuid,
        task_kind: &str,
        args: &[String],
    ) -> Result<crate::util::tasks::TaskRecord, CommandError> {
        self.server
            .lock()
            .await
            .context()
            .queue_named_task(clientid, task_kind, args)
            .await
            .map_err(|err| CommandError::new(err.to_string()))
    }

    pub async fn task_result(&self, taskid: &Uuid) -> Option<crate::util::tasks::TaskRecord> {
        self.server.lock().await.context().task_result(taskid).await
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
    pub async fn new() -> Self {
        let server = HttpServer::new(ServerContext::new());
        Self {
            context: Arc::new(CommandContext {
                server: Arc::new(Mutex::new(server)),
            }),
            handlers: commands::registry(),
        }
    }

    pub fn parse_command(cmd: &str) -> Result<ParsedCommand, CommandError> {
        parser::parse_command(cmd).map_err(CommandError::new)
    }

    pub async fn handle(&self, command: ParsedCommand) -> Result<CommandOutput, CommandError> {
        match command {
            ParsedCommand::Exit => process::exit(0),
            ParsedCommand::Server(_) => self.execute("server", command).await,
            ParsedCommand::Implants(_) => self.execute("implants", command).await,
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

    pub fn context(&self) -> Arc<CommandContext> {
        self.context.clone()
    }
}

pub fn parse_uuid(input: &str) -> Result<Uuid, CommandError> {
    input
        .parse::<Uuid>()
        .map_err(|_| CommandError::new("Invalid UUID"))
}

pub fn show_implant_list(records: &[crate::util::implants::ImplantRecord]) -> CommandOutput {
    CommandOutput::lines(records.iter().map(output::format_implant_list).collect())
}

pub fn show_implant_info(record: &crate::util::implants::ImplantRecord) -> CommandOutput {
    CommandOutput::lines(output::format_implant_info(record))
}

pub fn show_task_result(record: &crate::util::tasks::TaskRecord) -> CommandOutput {
    CommandOutput::lines(output::format_task_result(record))
}

pub fn info(message: &str) -> CommandOutput {
    CommandOutput::line(message)
}

pub fn good(message: &str) -> CommandOutput {
    CommandOutput::line(message)
}

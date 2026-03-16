//! Registers the concrete command executors available to the operator console.

use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use super::dispatcher::CommandExecutor;
use crate::core::command::{
    dispatcher::{
        good, info, parse_uuid, show_implant_info, show_implant_list, show_task_result,
        CommandContext, CommandError, CommandOutput,
    },
    parser::{ImplantCommand, ParsedCommand, ServerCommand, TaskCommand},
};

pub fn registry() -> HashMap<&'static str, CommandExecutor> {
    let mut handlers = HashMap::new();
    handlers.insert("server", execute_server as CommandExecutor);
    handlers.insert("implants", execute_implants as CommandExecutor);
    handlers.insert("tasks", execute_tasks as CommandExecutor);
    handlers
}

type CommandFuture = Pin<Box<dyn Future<Output = Result<CommandOutput, CommandError>> + Send>>;

fn execute_server(context: Arc<CommandContext>, command: ParsedCommand) -> CommandFuture {
    Box::pin(async move {
        let ParsedCommand::Server(command) = command else {
            return Err(CommandError::new("Invalid server command"));
        };

        match command {
            ServerCommand::HttpServer { action } => match action.as_str() {
                "start" => context.start_server().await,
                "stop" => Ok(context.stop_server().await),
                _ => Err(CommandError::new("Usage: httpserver start|stop")),
            },
        }
    })
}

fn execute_implants(context: Arc<CommandContext>, command: ParsedCommand) -> CommandFuture {
    Box::pin(async move {
        let ParsedCommand::Implants(command) = command else {
            return Err(CommandError::new("Invalid implants command"));
        };

        match command {
            ImplantCommand::List => {
                let implants = context.list_implants().await;
                if implants.is_empty() {
                    return Ok(info("No implants registered"));
                }

                Ok(show_implant_list(&implants))
            }
            ImplantCommand::Info { clientid } => {
                let clientid = parse_uuid(&clientid)?;
                match context.implant_info(&clientid).await {
                    Some(implant) => Ok(show_implant_info(&implant)),
                    None => Err(CommandError::new("Implant not found")),
                }
            }
        }
    })
}

fn execute_tasks(context: Arc<CommandContext>, command: ParsedCommand) -> CommandFuture {
    Box::pin(async move {
        let ParsedCommand::Tasks(command) = command else {
            return Err(CommandError::new("Invalid task command"));
        };

        match command {
            TaskCommand::Queue {
                clientid,
                task_kind,
                args,
            } => {
                let clientid = parse_uuid(&clientid)?;
                let task = context.queue_task(clientid, &task_kind, &args).await?;
                Ok(good(&format!(
                    "Queued task {} for {}",
                    task.task_id, task.clientid
                )))
            }
            TaskCommand::Result { task_id } => {
                let task_id = parse_uuid(&task_id)?;
                match context.task_result(&task_id).await {
                    Some(task) => Ok(show_task_result(&task)),
                    None => Err(CommandError::new("Task not found")),
                }
            }
        }
    })
}

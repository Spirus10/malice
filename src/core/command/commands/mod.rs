//! Registers the concrete command executors available to the operator console.

use std::{collections::HashMap, sync::Arc};

use super::dispatcher::{CommandExecutor, CommandFuture};
use crate::core::command::{
    dispatcher::{
        good, info, parse_uuid, show_implant_info, show_implant_list, show_task_result,
        CommandContext, CommandError, CommandOutput,
    },
    parser::{ImplantCommand, ParsedCommand, PluginCommand, ServerCommand, TaskCommand},
};

pub fn registry() -> HashMap<&'static str, CommandExecutor> {
    let mut handlers = HashMap::new();
    handlers.insert("server", execute_server as CommandExecutor);
    handlers.insert("implants", execute_implants as CommandExecutor);
    handlers.insert("plugins", execute_plugins as CommandExecutor);
    handlers.insert("tasks", execute_tasks as CommandExecutor);
    handlers
}

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

fn execute_plugins(context: Arc<CommandContext>, command: ParsedCommand) -> CommandFuture {
    Box::pin(async move {
        let ParsedCommand::Plugins(command) = command else {
            return Err(CommandError::new("Invalid plugin command"));
        };

        match command {
            PluginCommand::List => {
                let plugins = context.list_plugins().await?;
                let load_errors = context.plugin_load_errors().await;
                if plugins.is_empty() {
                    if load_errors.is_empty() {
                        return Ok(info("No plugins installed"));
                    }

                    let mut lines = vec!["No plugins installed".to_string()];
                    lines.extend(
                        load_errors
                            .into_iter()
                            .map(|error| format!("load error: {error}")),
                    );
                    return Ok(CommandOutput::lines(lines));
                }

                let mut lines: Vec<String> = plugins
                    .into_iter()
                    .map(|plugin| {
                        format!(
                            "{} {} [{}] {}",
                            plugin.plugin_id,
                            plugin.version,
                            if plugin.active { "active" } else { "inactive" },
                            plugin.install_path.display()
                        )
                    })
                    .collect();
                lines.extend(
                    load_errors
                        .into_iter()
                        .map(|error| format!("load error: {error}")),
                );
                Ok(CommandOutput::lines(lines))
            }
            PluginCommand::Install { source } => {
                let plugin = context.install_plugin(&source).await?;
                Ok(good(&format!(
                    "Installed plugin {} {}",
                    plugin.plugin_id, plugin.version
                )))
            }
            PluginCommand::Activate { plugin_id, version } => {
                let plugin = context
                    .activate_plugin(&plugin_id, version.as_deref())
                    .await?;
                Ok(good(&format!(
                    "Activated plugin {} {}",
                    plugin.plugin_id, plugin.version
                )))
            }
            PluginCommand::Deactivate { plugin_id } => {
                context.deactivate_plugin(&plugin_id).await?;
                Ok(good(&format!("Deactivated plugin {plugin_id}")))
            }
            PluginCommand::Remove { plugin_id, version } => {
                context.remove_plugin(&plugin_id, &version).await?;
                Ok(good(&format!("Removed plugin {} {}", plugin_id, version)))
            }
            PluginCommand::Inspect { plugin_id, version } => {
                let plugin = context
                    .inspect_plugin(&plugin_id, version.as_deref())
                    .await?;
                Ok(CommandOutput::lines(vec![
                    format!("plugin_id: {}", plugin.descriptor.plugin_id),
                    format!("version: {}", plugin.descriptor.version),
                    format!("package_id: {}", plugin.descriptor.package_id),
                    format!("active: {}", plugin.active),
                    format!("package_root: {}", plugin.package_root.display()),
                    format!("schema_version: {}", plugin.manifest.schema_version),
                    format!("display_name: {}", plugin.manifest.display_name),
                    format!("description: {}", plugin.manifest.description),
                    format!("implant_type: {}", plugin.manifest.implant_type),
                    format!("family: {}", plugin.manifest.family),
                    format!(
                        "protocol_versions: {}",
                        plugin
                            .manifest
                            .protocol_versions
                            .iter()
                            .map(u32::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    format!("capabilities: {}", plugin.manifest.capabilities.join(", ")),
                ]))
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

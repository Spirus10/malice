use std::{future::Future, pin::Pin, sync::Arc};

use crate::util::command::{
    dispatcher::{good, parse_uuid, show_task_result, CommandContext, CommandError},
    parser::{ParsedCommand, TaskCommand},
};

pub fn execute(context: Arc<CommandContext>, command: ParsedCommand) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + Send>> {
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
                good(&format!("Queued task {} for {}", task.task_id, task.clientid));
                Ok(())
            }
            TaskCommand::Result { task_id } => {
                let task_id = parse_uuid(&task_id)?;
                match context.task_result(&task_id).await {
                    Some(task) => {
                        show_task_result(&task);
                        Ok(())
                    }
                    None => Err(CommandError::new("Task not found")),
                }
            }
        }
    })
}

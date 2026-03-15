use std::{future::Future, pin::Pin, sync::Arc};

use crate::util::command::{
    dispatcher::{CommandContext, CommandError},
    parser::{ParsedCommand, ServerCommand},
};

pub fn execute(
    context: Arc<CommandContext>,
    command: ParsedCommand,
) -> Pin<
    Box<
        dyn Future<Output = Result<crate::util::command::dispatcher::CommandOutput, CommandError>>
            + Send,
    >,
> {
    Box::pin(async move {
        let ParsedCommand::Server(command) = command else {
            return Err(CommandError::new("Invalid server command"));
        };

        match command {
            ServerCommand::TcpServer { action } => match action.as_str() {
                "start" => context.start_server().await,
                "stop" => Ok(context.stop_server().await),
                _ => Err(CommandError::new("Usage: tcpserver start|stop")),
            },
        }
    })
}

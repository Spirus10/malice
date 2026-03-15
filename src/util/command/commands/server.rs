use std::{future::Future, pin::Pin, sync::Arc};

use crate::util::command::{
    dispatcher::{good, info, CommandContext, CommandError},
    parser::{ParsedCommand, ServerCommand},
};

pub fn execute(context: Arc<CommandContext>, command: ParsedCommand) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + Send>> {
    Box::pin(async move {
        let ParsedCommand::Server(command) = command else {
            return Err(CommandError::new("Invalid server command"));
        };

        match command {
            ServerCommand::TcpServer { action } => match action.as_str() {
                "start" => {
                    context.start_server().await?;
                    good("HTTP server started on 127.0.0.1:42069");
                    Ok(())
                }
                "stop" => {
                    context.stop_server().await;
                    info("HTTP server stopped");
                    Ok(())
                }
                _ => Err(CommandError::new("Usage: tcpserver start|stop")),
            },
        }
    })
}

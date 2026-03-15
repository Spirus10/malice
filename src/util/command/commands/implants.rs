use std::{future::Future, pin::Pin, sync::Arc};

use crate::util::command::{
    dispatcher::{info, parse_uuid, show_implant_info, show_implant_list, CommandContext, CommandError},
    parser::{ImplantCommand, ParsedCommand},
};

pub fn execute(context: Arc<CommandContext>, command: ParsedCommand) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + Send>> {
    Box::pin(async move {
        let ParsedCommand::Implants(command) = command else {
            return Err(CommandError::new("Invalid implants command"));
        };

        match command {
            ImplantCommand::List => {
                let implants = context.list_implants().await;
                if implants.is_empty() {
                    info("No implants registered");
                    return Ok(());
                }

                show_implant_list(&implants);
                Ok(())
            }
            ImplantCommand::Info { clientid } => {
                let clientid = parse_uuid(&clientid)?;
                match context.implant_info(&clientid).await {
                    Some(implant) => {
                        show_implant_info(&implant);
                        Ok(())
                    }
                    None => Err(CommandError::new("Implant not found")),
                }
            }
        }
    })
}

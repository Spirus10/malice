use std::io;
use std::io::Write;
use std::error::Error;

mod util;
use util::{
    command::CommandHandler,
    logger,
};



#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let command_handler = CommandHandler::new().await;
    
    loop {

        print!("[malice]> ");
        io::stdout().flush().unwrap();
        let mut input: String = String::new();

        let cmd_size: usize = std::io::stdin().read_line(&mut input).unwrap();
        input.pop();

        if cmd_size == 1 { continue; }

        else { 

            match CommandHandler::parse_command(&input) {
                Ok(cmd) => match command_handler.handle(cmd).await {
                    Ok(_) => continue,
                    Err(e) => logger::bad(e.unwrap()),
                },
                Err(e) => logger::bad(e.unwrap()),
            }
        }
    } 
}

use std::io;
use std::io::Write;
use std::error::Error;
use std::sync::{Arc, Mutex, mpsc};

mod util;
use util::{
    command::CommandHandler,
    logger,
};



#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let mut command_handler = CommandHandler::new().await;
    
    loop {

        print!("[malice]> ");
        io::stdout().flush().unwrap();
        let mut input: String = String::new();

        let cmd_size: usize = std::io::stdin().read_line(&mut input).unwrap();
        input.pop();

        if cmd_size == 1 { continue; }

        else { 

            let cmd = CommandHandler::parse_command(&input);
            match command_handler.handle(&cmd).await {
                Ok(_) => continue,
                Err(e) => logger::bad(e.unwrap()),
            }
        }
    } 
}

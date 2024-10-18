use std::fmt;
use std::process;
use std::error::Error;
use std::sync::{Arc, mpsc};

use tokio::task;
use tokio::sync::Mutex;

use tokio_util::sync::CancellationToken;

use super::logger;
use super::httpserver::{HttpServer};

use uuid::Uuid;

// ------------------- Command Error -----------------------------
// CommandError must derive Debug to implement Error
#[derive(Debug)]
pub struct CommandError {
    details: String
}

impl CommandError {
    pub fn new(msg: &str, cmd: &str) -> CommandError {
        let mut ret = msg.to_owned();
        ret.push_str(cmd);
        CommandError{details: ret}
    }
    pub fn unwrap(&self) -> &str {
        return &self.details.as_str()
    }
}

impl Error for CommandError {
    fn description(&self) -> &str {
        &self.details
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.details)
    }
}

// ------------------- Command Error -----------------------------

// ------------------- Command Datatype and Handling -------------

#[derive(Debug)]
pub enum Command {
    TcpServer(String),
    Exit,
    Unknown(String),
    Usage(String)
}

pub struct CommandHandler {
    http_server: HttpServer,
}

impl CommandHandler {
    
    pub async fn new() -> Self {
        Self {
            http_server: HttpServer::new().await.unwrap(),
        }
    }
    pub fn parse_command(cmd: &str) -> Command {

        let mut parts = cmd.split_whitespace();
        let command = parts.next().unwrap_or("");
        
        let args : Vec<String> = parts.map(|s: &str| s.to_string()).collect();

        match command {
            "tcpserver" => {
                if args.len() == 1 {
                    return Command::TcpServer(args[0].to_string());
                } else {
                    return Command::Usage("Usage: tcpserver start|stop".to_string());
                }
                
            },
            "exit" => Command::Exit,
            _ => Command::Unknown(command.to_string()),
        }
    }

    pub async fn handle(&mut self, command: &Command) -> Result<(), CommandError>{

        match command {
            Command::TcpServer(arg) => {
                match arg.as_str() {
                    "start" => {
                        self.http_server.start().await.unwrap();
                        Ok(())
                    },
                    "stop" => {
                        self.http_server.close().await;
                        Ok(())
                    },
                    _ => { logger::info("Usage: tcpserver start|stop"); Ok(()) },
                }
            },
            Command::Exit => { process::exit(0); }
            Command::Unknown(s) => {
                Err(CommandError::new("Oh no! Command unknown: ", s))
            },
            Command::Usage(usage) => {
                logger::info(usage);
                Ok(())
            },
        }

    }
}

// ------------------- Command Datatype and Handling -------------
#[derive(Debug, Clone)]
pub enum ParsedCommand {
    Server(ServerCommand),
    Implants(ImplantCommand),
    Tasks(TaskCommand),
    Exit,
}

#[derive(Debug, Clone)]
pub enum ServerCommand {
    TcpServer { action: String },
}

#[derive(Debug, Clone)]
pub enum ImplantCommand {
    List,
    Info { clientid: String },
}

#[derive(Debug, Clone)]
pub enum TaskCommand {
    Queue {
        clientid: String,
        task_kind: String,
        args: Vec<String>,
    },
    Result {
        task_id: String,
    },
}

pub fn parse_command(input: &str) -> Result<ParsedCommand, String> {
    let mut parts = input.split_whitespace();
    let command = parts.next().unwrap_or("");
    let args: Vec<String> = parts.map(str::to_string).collect();

    match command {
        "tcpserver" => match args.as_slice() {
            [action] => Ok(ParsedCommand::Server(ServerCommand::TcpServer {
                action: action.clone(),
            })),
            _ => Err("Usage: tcpserver start|stop".to_string()),
        },
        "implants" => match args.as_slice() {
            [subcommand] if subcommand == "list" => {
                Ok(ParsedCommand::Implants(ImplantCommand::List))
            }
            [subcommand, clientid] if subcommand == "info" => {
                Ok(ParsedCommand::Implants(ImplantCommand::Info {
                    clientid: clientid.clone(),
                }))
            }
            _ => Err("Usage: implants list | implants info <clientid>".to_string()),
        },
        "task" => match args.as_slice() {
            [subcommand, clientid, task_kind, rest @ ..] if subcommand == "queue" => {
                Ok(ParsedCommand::Tasks(TaskCommand::Queue {
                    clientid: clientid.clone(),
                    task_kind: task_kind.clone(),
                    args: rest.to_vec(),
                }))
            }
            [subcommand, task_id] if subcommand == "result" => {
                Ok(ParsedCommand::Tasks(TaskCommand::Result {
                    task_id: task_id.clone(),
                }))
            }
            _ => Err(
                "Usage: task queue <clientid> <task-kind> [args..] | task result <taskid>"
                    .to_string(),
            ),
        },
        "exit" => Ok(ParsedCommand::Exit),
        _ => Err(format!("Unknown command: {command}")),
    }
}

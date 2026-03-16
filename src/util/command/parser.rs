//! Parses operator command lines into structured command enums.

#[derive(Debug, Clone)]
pub enum ParsedCommand {
    Server(ServerCommand),
    Implants(ImplantCommand),
    Tasks(TaskCommand),
    Exit,
}

#[derive(Debug, Clone)]
pub enum ServerCommand {
    HttpServer { action: String },
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

pub fn tokenize_command_line(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars();
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some(active_quote) if ch == active_quote => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err("Unterminated quoted string".to_string());
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Parses one operator command line into the internal command model.
///
/// @param input Raw command line entered by the operator.
/// @return Parsed command on success, or a usage/error message on failure.
pub fn parse_command(input: &str) -> Result<ParsedCommand, String> {
    let tokens = tokenize_command_line(input)?;
    let mut parts = tokens.into_iter();
    let command = parts.next().unwrap_or_default();
    let args: Vec<String> = parts.collect();

    match command.as_str() {
        "httpserver" => match args.as_slice() {
            [action] => Ok(ParsedCommand::Server(ServerCommand::HttpServer {
                action: action.clone(),
            })),
            _ => Err("Usage: httpserver start|stop".to_string()),
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

#[cfg(test)]
mod tests {
    use super::{parse_command, ParsedCommand, TaskCommand};

    #[test]
    fn parses_quoted_task_arguments() {
        let parsed =
            parse_command(r#"task queue selected execute "cmd.exe /c echo hello world""#).unwrap();

        match parsed {
            ParsedCommand::Tasks(TaskCommand::Queue {
                clientid,
                task_kind,
                args,
            }) => {
                assert_eq!(clientid, "selected");
                assert_eq!(task_kind, "execute");
                assert_eq!(args, vec!["cmd.exe /c echo hello world"]);
            }
            _ => panic!("unexpected command shape"),
        }
    }

    #[test]
    fn rejects_unterminated_quotes() {
        let error = parse_command(r#"task queue selected upload "C:\Temp\a.txt"#).unwrap_err();
        assert_eq!(error, "Unterminated quoted string");
    }

    #[test]
    fn preserves_windows_paths_inside_quotes() {
        let parsed = parse_command(r#"task queue selected ls "C:\Users\wammu\Documents""#).unwrap();

        match parsed {
            ParsedCommand::Tasks(TaskCommand::Queue { args, .. }) => {
                assert_eq!(args, vec![r#"C:\Users\wammu\Documents"#]);
            }
            _ => panic!("unexpected command shape"),
        }
    }
}

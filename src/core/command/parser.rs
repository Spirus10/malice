//! Parses operator command lines into structured command enums.
//!
//! Parsing stages:
//!
//!   raw line
//!      -> tokenize_command_line
//!         - preserves quoted substrings as one token
//!         - strips the surrounding quote characters
//!      -> parse_command
//!         - dispatches on the first token
//!         - validates argument shape for that command family
//!         - returns a typed `ParsedCommand`
//!
//! Example:
//!
//!   task queue selected execute "cmd.exe /c echo hello"
//!
//!      -> ["task", "queue", "selected", "execute", "cmd.exe /c echo hello"]
//!      -> ParsedCommand::Tasks(TaskCommand::Queue { ... })

#[derive(Debug, Clone)]
pub enum ParsedCommand {
    Server(ServerCommand),
    Implants(ImplantCommand),
    Plugins(PluginCommand),
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
pub enum PluginCommand {
    List,
    Install {
        source: String,
    },
    Activate {
        plugin_id: String,
        version: Option<String>,
    },
    Deactivate {
        plugin_id: String,
    },
    Remove {
        plugin_id: String,
        version: String,
    },
    Inspect {
        plugin_id: String,
        version: Option<String>,
    },
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
/// Decision tree:
///
///   first token
///      -> "httpserver" -> ServerCommand
///      -> "implants"   -> ImplantCommand
///      -> "plugins"    -> PluginCommand
///      -> "task"       -> TaskCommand
///      -> "exit"       -> ParsedCommand::Exit
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
        "plugins" => match args.as_slice() {
            [subcommand] if subcommand == "list" => {
                Ok(ParsedCommand::Plugins(PluginCommand::List))
            }
            [subcommand, source] if subcommand == "install" => {
                Ok(ParsedCommand::Plugins(PluginCommand::Install {
                    source: source.clone(),
                }))
            }
            [subcommand, plugin_id] if subcommand == "activate" => {
                Ok(ParsedCommand::Plugins(PluginCommand::Activate {
                    plugin_id: plugin_id.clone(),
                    version: None,
                }))
            }
            [subcommand, plugin_id, version] if subcommand == "activate" => {
                Ok(ParsedCommand::Plugins(PluginCommand::Activate {
                    plugin_id: plugin_id.clone(),
                    version: Some(version.clone()),
                }))
            }
            [subcommand, plugin_id] if subcommand == "deactivate" => {
                Ok(ParsedCommand::Plugins(PluginCommand::Deactivate {
                    plugin_id: plugin_id.clone(),
                }))
            }
            [subcommand, plugin_id, version] if subcommand == "remove" => {
                Ok(ParsedCommand::Plugins(PluginCommand::Remove {
                    plugin_id: plugin_id.clone(),
                    version: version.clone(),
                }))
            }
            [subcommand, plugin_id] if subcommand == "inspect" => {
                Ok(ParsedCommand::Plugins(PluginCommand::Inspect {
                    plugin_id: plugin_id.clone(),
                    version: None,
                }))
            }
            [subcommand, plugin_id, version] if subcommand == "inspect" => {
                Ok(ParsedCommand::Plugins(PluginCommand::Inspect {
                    plugin_id: plugin_id.clone(),
                    version: Some(version.clone()),
                }))
            }
            _ => Err(
                "Usage: plugins list | plugins install <path> | plugins activate <id> [version] | plugins deactivate <id> | plugins remove <id> <version> | plugins inspect <id> [version]"
                    .to_string(),
            ),
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
    use super::{parse_command, ParsedCommand, PluginCommand, TaskCommand};

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

    #[test]
    fn parses_plugin_activate_with_optional_version() {
        let parsed = parse_command("plugins activate zant 0.1.0").unwrap();

        match parsed {
            ParsedCommand::Plugins(PluginCommand::Activate { plugin_id, version }) => {
                assert_eq!(plugin_id, "zant");
                assert_eq!(version.as_deref(), Some("0.1.0"));
            }
            _ => panic!("unexpected command shape"),
        }
    }
}

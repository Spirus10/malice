//! Registers the concrete command executors available to the operator console.

use std::collections::HashMap;

use super::dispatcher::CommandExecutor;

mod implants;
mod server;
mod tasks;

pub fn registry() -> HashMap<&'static str, CommandExecutor> {
    let mut handlers = HashMap::new();
    handlers.insert("server", server::execute as CommandExecutor);
    handlers.insert("implants", implants::execute as CommandExecutor);
    handlers.insert("tasks", tasks::execute as CommandExecutor);
    handlers
}

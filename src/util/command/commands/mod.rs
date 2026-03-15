mod implants;
mod server;
mod tasks;

use std::collections::HashMap;

use super::dispatcher::CommandExecutor;

pub fn registry() -> HashMap<&'static str, CommandExecutor> {
    let mut handlers = HashMap::new();
    handlers.insert("server", server::execute as CommandExecutor);
    handlers.insert("implants", implants::execute as CommandExecutor);
    handlers.insert("tasks", tasks::execute as CommandExecutor);
    handlers
}

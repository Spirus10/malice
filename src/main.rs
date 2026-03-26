//! Starts the terminal UI and routes fatal startup/runtime errors to stderr.

use std::error::Error;

mod core;
mod ui;
use crate::core::logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Some(addr_str) = std::env::args().nth(1) {
        let addr: std::net::SocketAddr = addr_str
            .parse()
            .expect("invalid bind address; expected format like 127.0.0.1:42069");
        crate::core::httpserver::set_bind_addr(addr);
    }

    if let Err(err) = ui::run().await {
        logger::bad(&err.to_string());
    }
    Ok(())
}

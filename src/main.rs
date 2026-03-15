use std::error::Error;

mod ui;
mod util;
use util::logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Err(err) = ui::run().await {
        logger::bad(&err.to_string());
    }
    Ok(())
}

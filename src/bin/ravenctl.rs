// Raven Control CLI - Command line interface for managing Raven data collection

use raven::controller;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    controller::run().await
}

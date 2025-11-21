// Project Raven - Market Data Subscription Server

use raven::{common::error::RavenResult, server::app::run};

#[tokio::main]
async fn main() -> RavenResult<()> {
    run().await
}

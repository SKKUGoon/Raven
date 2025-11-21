// Project Raven - Market Data Subscription Server

use raven::{app, error::RavenResult};

#[tokio::main]
async fn main() -> RavenResult<()> {
    app::run().await
}

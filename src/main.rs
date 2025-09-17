// Project Raven - Market Data Subscription Server
// The Night's Watch begins here - "Crown the king"

use raven::{app, error::RavenResult};

#[tokio::main]
async fn main() -> RavenResult<()> {
    app::run().await
}

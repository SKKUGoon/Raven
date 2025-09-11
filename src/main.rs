// Project Raven - Market Data Subscription Server
// The Night's Watch begins here - "Crown the king"

mod app;

use market_data_subscription_server::error::RavenResult;

#[tokio::main]
async fn main() -> RavenResult<()> {
    app::run().await
}

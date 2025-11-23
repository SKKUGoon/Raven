// Project Raven - Market Data Subscription Server
// The Raven Server begins here

pub mod args;
pub mod server;
pub mod shutdown;
pub mod startup;

use crate::common::error::RavenResult;

/// Main application entry point and coordination
pub async fn run() -> RavenResult<()> {
    server::Server::builder().build().await?.run().await
}

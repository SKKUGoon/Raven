//! Deribit BTC options **trades** microservice (every trade execution).

#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::service::RavenService;
use raven::source::deribit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;
    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_deribit_trades
    )
    .parse()?;
    let svc = deribit::new_trades_service(
        settings.deribit.ws_url.clone(),
        settings.deribit.channel_capacity,
    );
    let raven = RavenService::new("DeribitTrades", svc.clone());
    raven.serve_with_market_data(addr, svc).await?;
    Ok(())
}

//! Binance Futures **mark price / funding rate** WebSocket stream.

#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::service::RavenService;
use raven::source::binance::futures::funding::FundingRateService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;
    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_binance_futures_funding
    )
    .parse()?;
    let service_impl = FundingRateService::new(settings.binance_rest.channel_capacity);
    let raven = RavenService::new("BinanceFuturesFunding", service_impl.clone());
    raven.serve_with_market_data(addr, service_impl).await?;
    Ok(())
}

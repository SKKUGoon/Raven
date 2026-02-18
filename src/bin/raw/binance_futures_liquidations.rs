//! Binance Futures **all-market liquidation** WebSocket stream.

#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::service::RavenService;
use raven::source::binance::futures::liquidation::LiquidationService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;
    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_binance_futures_liquidations
    )
    .parse()?;
    let service_impl = LiquidationService::new(settings.binance_rest.channel_capacity);
    let raven = RavenService::new("BinanceFuturesLiquidations", service_impl.clone());
    raven.serve_with_market_data(addr, service_impl).await?;
    Ok(())
}

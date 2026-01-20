#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::service::RavenService;
use raven::source::binance::futures::klines::BinanceFuturesKlinesService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_binance_futures_klines
    )
    .parse()?;

    let service_impl = BinanceFuturesKlinesService::new(&settings);
    let raven = RavenService::new("BinanceFuturesKlines", service_impl.clone());
    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}

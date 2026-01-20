#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::service::RavenService;
use raven::source::binance::futures;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    common::init_logging(&settings);

    let addr =
        format!("{}:{}", settings.server.host, settings.server.port_binance_futures).parse()?;
    let service_impl = futures::new();
    let raven = RavenService::new("BinanceFutures", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}

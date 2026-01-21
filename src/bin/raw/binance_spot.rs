#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::service::RavenService;
use raven::source::binance::spot;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    common::init_logging(&settings);

    let addr = format!("{}:{}", settings.server.host, settings.server.port_binance_spot).parse()?;
    let service_impl = spot::new();
    let raven = RavenService::new("BinanceSpot", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}

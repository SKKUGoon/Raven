//! Binance European Options **ticker** REST poller (OI + IV → OptionsTicker).
//!
//! Outputs OptionsTicker with venue=BINANCE_OPTIONS. Aggregate with Deribit
//! data downstream for combined positioning.

#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::service::RavenService;
use raven::source::binance::options::BinanceOptionsService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;
    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_binance_options
    )
    .parse()?;
    let service_impl = BinanceOptionsService::new(&settings.binance_rest);
    let raven = RavenService::new("BinanceOptions", service_impl.clone());
    raven.serve_with_market_data(addr, service_impl).await?;
    Ok(())
}

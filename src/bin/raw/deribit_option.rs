//! Deribit BTC options **ticker** microservice (open interest, IV, mark price).

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
        settings.server.host, settings.server.port_deribit_ticker
    )
    .parse()?;
    let svc = deribit::new_ticker_service(
        settings.deribit.ws_url.clone(),
        settings.deribit.channel_capacity,
    );
    let raven = RavenService::new("DeribitTicker", svc.clone());
    raven.serve_with_market_data(addr, svc).await?;
    Ok(())
}

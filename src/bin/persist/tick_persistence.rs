#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::db::influx;
use raven::service::RavenService;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_tick_persistence
    )
    .parse()?;

    let spot_upstream = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_binance_spot
    );

    let futures_upstream = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_binance_futures
    );

    let mut upstreams = HashMap::new();
    upstreams.insert("BINANCE_SPOT".to_string(), spot_upstream.clone());
    upstreams.insert("BINANCE_FUTURES".to_string(), futures_upstream);

    let service_impl = influx::new(spot_upstream, upstreams, settings.influx.clone());
    let raven = RavenService::new("TickPersistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}

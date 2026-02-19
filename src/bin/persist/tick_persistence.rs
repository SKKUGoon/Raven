#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::db::influx;
use raven::service::{RavenService, StreamDataType, StreamKey};
use raven::utils::service_registry::client_host;
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

    let h = client_host(&settings.server.host);

    let spot_upstream = format!("http://{}:{}", h, settings.server.port_binance_spot);
    let futures_upstream = format!("http://{}:{}", h, settings.server.port_binance_futures);

    let mut upstreams = HashMap::new();
    upstreams.insert("BINANCE_SPOT".to_string(), spot_upstream.clone());
    upstreams.insert("BINANCE_FUTURES".to_string(), futures_upstream);

    // Data-type-specific upstreams (composite key: "VENUE:DATA_TYPE")
    upstreams.insert(
        "BINANCE_FUTURES:FUNDING".to_string(),
        format!(
            "http://{}:{}",
            h, settings.server.port_binance_futures_funding
        ),
    );
    upstreams.insert(
        "BINANCE_FUTURES:LIQUIDATION".to_string(),
        format!(
            "http://{}:{}",
            h, settings.server.port_binance_futures_liquidations
        ),
    );
    upstreams.insert(
        "BINANCE_FUTURES:OPEN_INTEREST".to_string(),
        format!("http://{}:{}", h, settings.server.port_binance_futures_oi),
    );
    upstreams.insert(
        "BINANCE_OPTIONS:TICKER".to_string(),
        format!("http://{}:{}", h, settings.server.port_binance_options),
    );
    upstreams.insert(
        "DERIBIT:TICKER".to_string(),
        format!("http://{}:{}", h, settings.server.port_deribit_ticker),
    );
    upstreams.insert(
        "DERIBIT:TRADE".to_string(),
        format!("http://{}:{}", h, settings.server.port_deribit_trades),
    );
    upstreams.insert(
        "DERIBIT:PRICE_INDEX".to_string(),
        format!("http://{}:{}", h, settings.server.port_deribit_index),
    );

    let service_impl = influx::new(spot_upstream, upstreams, settings.influx.clone());

    // Auto-start wildcard streams for all-market services.
    // These services stream data for all symbols; persistence subscribes with "*".
    let auto_streams = [
        ("BINANCE_FUTURES", StreamDataType::Funding),
        ("BINANCE_FUTURES", StreamDataType::Liquidation),
        ("BINANCE_FUTURES", StreamDataType::OpenInterest),
        ("BINANCE_OPTIONS", StreamDataType::Ticker),
        ("DERIBIT", StreamDataType::Ticker),
        ("DERIBIT", StreamDataType::Trade),
        ("DERIBIT", StreamDataType::PriceIndex),
    ];

    for (venue, data_type) in auto_streams {
        service_impl
            .ensure_stream(StreamKey {
                symbol: "*".to_string(),
                venue: Some(venue.to_string()),
                data_type,
            })
            .await;
    }

    let raven = RavenService::new("TickPersistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}

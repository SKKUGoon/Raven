#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::db::timescale;
use raven::service::{RavenService, StreamDataType, StreamKey};

fn resolve_symbols(settings: &Settings) -> Vec<String> {
    if !settings.binance_klines.symbols.is_empty() {
        let mut out: Vec<String> = settings
            .binance_klines
            .symbols
            .iter()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        out.sort();
        out.dedup();
        return out;
    }

    // Fall back to routing.symbol_map BINANCE_FUTURES entries.
    let mut out = Vec::new();
    for venue_map in settings.routing.symbol_map.values() {
        if let Some(sym) = venue_map.get("BINANCE_FUTURES") {
            let s = sym.trim().to_uppercase();
            if !s.is_empty() {
                out.push(s);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    common::init_logging(&settings);

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_kline_persistence
    )
    .parse()?;

    let upstream_url = format!(
        "http://{}:{}",
        settings.server.host, settings.server.port_binance_futures_klines
    );

    let symbols = resolve_symbols(&settings);
    if symbols.is_empty() {
        tracing::warn!(
            "kline_persistence: no symbols configured (set `binance_klines.symbols` or add futures entries to `routing.symbol_map`)."
        );
    }

    let service_impl = timescale::new_kline(upstream_url, settings.timescale.clone()).await?;

    // Start persistence for all configured symbols immediately.
    for sym in symbols {
        let key = StreamKey {
            symbol: sym,
            venue: Some("BINANCE_FUTURES".to_string()),
            data_type: StreamDataType::Candle,
        };
        service_impl.ensure_stream(key).await;
    }

    let raven = RavenService::new("KlinePersistence", service_impl.clone());
    raven.serve(addr).await?;

    Ok(())
}

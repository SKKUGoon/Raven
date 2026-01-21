#[path = "../common/mod.rs"]
mod common;

use raven::config::Settings;
use raven::db::timescale;
use raven::service::{RavenService, StreamDataType, StreamKey};
use raven::utils::service_registry;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ExchangeInfoResponse {
    symbols: Vec<ExchangeSymbol>,
}

#[derive(Debug, Deserialize)]
struct ExchangeSymbol {
    symbol: String,
    #[serde(rename = "contractType")]
    contract_type: String,
    status: String,
    #[serde(rename = "quoteAsset")]
    quote_asset: String,
}

async fn resolve_symbols(settings: &Settings) -> Vec<String> {
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

    if let Some(symbols) = fetch_binance_futures_symbols().await {
        if !symbols.is_empty() {
            return symbols;
        }
    }

    // Fall back to routing.symbol_map BINANCE_FUTURES entries.
    collect_routing_symbols(settings)
}

fn collect_routing_symbols(settings: &Settings) -> Vec<String> {
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

async fn fetch_binance_futures_symbols() -> Option<Vec<String>> {
    let url = "https://fapi.binance.com/fapi/v1/exchangeInfo";
    let response = reqwest::get(url).await.ok()?;
    let payload: ExchangeInfoResponse = response.json().await.ok()?;

    let mut out: Vec<String> = payload
        .symbols
        .into_iter()
        .filter(|s| {
            s.contract_type.eq_ignore_ascii_case("PERPETUAL")
                && s.status.eq_ignore_ascii_case("TRADING")
                && s.quote_asset.eq_ignore_ascii_case("USDT")
        })
        .map(|s| s.symbol.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    out.sort();
    out.dedup();
    Some(out)
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

    let client_host = service_registry::client_host(&settings.server.host);
    let upstream_url = format!(
        "http://{}:{}",
        client_host, settings.server.port_binance_futures_klines
    );

    let symbols = resolve_symbols(&settings).await;
    if symbols.is_empty() {
        tracing::warn!(
            "kline_persistence: no symbols configured (set `binance_klines.symbols` or add futures entries to `routing.symbol_map`)."
        );
    }

    let service_impl = timescale::new_kline(upstream_url, settings.timescale.clone()).await?;

    // Start a single wildcard stream for all klines.
    let key = StreamKey {
        symbol: "*".to_string(),
        venue: Some("BINANCE_FUTURES".to_string()),
        data_type: StreamDataType::Candle,
    };
    service_impl.ensure_stream(key).await;

    let raven = RavenService::new("KlinePersistence", service_impl.clone());
    raven.serve(addr).await?;

    Ok(())
}

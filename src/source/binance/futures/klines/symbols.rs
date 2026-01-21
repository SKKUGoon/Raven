use crate::config::Settings;
use serde::Deserialize;
use tracing::warn;

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

pub(super) async fn resolve_symbols(settings: &Settings) -> Vec<String> {
    // Highest priority: explicit list.
    if !settings.binance_klines.symbols.is_empty() {
        return settings
            .binance_klines
            .symbols
            .iter()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
    }

    // Otherwise: fetch full list from Binance Futures exchangeInfo.
    if let Some(symbols) = fetch_binance_futures_symbols().await {
        if !symbols.is_empty() {
            return symbols;
        }
    }

    // Fallback: collect all venue-specific symbols from routing.symbol_map for BINANCE_FUTURES.
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

    if out.is_empty() {
        warn!("Binance futures klines: exchangeInfo returned no matching symbols.");
    }

    out.sort();
    out.dedup();
    Some(out)
}

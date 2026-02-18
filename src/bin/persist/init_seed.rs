use raven::config::Settings;
use raven::db::timescale::RavenInitSeed;
use serde::Deserialize;
use std::collections::BTreeSet;

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

pub async fn collect_raven_init_seed(settings: &Settings) -> RavenInitSeed {
    let mut symbols: BTreeSet<String> = BTreeSet::new();
    let mut exchanges: BTreeSet<String> = BTreeSet::new();
    let mut intervals: BTreeSet<String> = BTreeSet::new();

    for venue_map in settings.routing.symbol_map.values() {
        for (venue, sym) in venue_map {
            let s = sym.trim().to_uppercase();
            if !s.is_empty() {
                symbols.insert(s);
            }
            let v = venue.trim().to_uppercase();
            if !v.is_empty() {
                exchanges.insert(v);
            }
        }
    }

    for sym in &settings.binance_klines.symbols {
        let s = sym.trim().to_uppercase();
        if !s.is_empty() {
            symbols.insert(s);
        }
    }

    for sym in &settings.binance_rest.symbols {
        let s = sym.trim().to_uppercase();
        if !s.is_empty() {
            symbols.insert(s);
        }
    }

    if let Some(futures_symbols) = fetch_binance_futures_symbols().await {
        for s in futures_symbols {
            symbols.insert(s);
        }
    }

    exchanges.insert("BINANCE_SPOT".to_string());
    exchanges.insert("BINANCE_FUTURES".to_string());
    exchanges.insert("BINANCE_OPTIONS".to_string());
    exchanges.insert("DERIBIT".to_string());

    intervals.insert(settings.binance_klines.interval.trim().to_string());
    intervals.insert("tib_small".to_string());
    intervals.insert("tib_large".to_string());
    intervals.insert("trb_small".to_string());
    intervals.insert("trb_large".to_string());
    intervals.insert("vib_small".to_string());
    intervals.insert("vib_large".to_string());
    intervals.insert("vpin".to_string());

    for profile in settings.tibs.profiles.keys() {
        let p = profile.trim();
        if p.is_empty() {
            continue;
        }
        let normalized = if p.starts_with("tib") {
            p.to_string()
        } else {
            format!("tib_{p}")
        };
        intervals.insert(normalized);
    }

    RavenInitSeed {
        symbols: symbols.into_iter().collect(),
        exchanges: exchanges.into_iter().collect(),
        intervals: intervals.into_iter().collect(),
    }
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

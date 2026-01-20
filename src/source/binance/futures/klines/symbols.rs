use crate::config::Settings;

pub(super) fn resolve_symbols(settings: &Settings) -> Vec<String> {
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

    // Otherwise: collect all venue-specific symbols from routing.symbol_map for BINANCE_FUTURES.
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

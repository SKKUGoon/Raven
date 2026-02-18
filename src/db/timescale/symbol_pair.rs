const KNOWN_QUOTES: &[&str] = &[
    "USDT", "USDC", "FDUSD", "BUSD", "TUSD", "USDP", "USD", "BTC", "ETH",
];

pub(super) fn parse_coin_quote(raw: &str) -> Option<(String, String)> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }

    if let Some((coin, quote)) = t.split_once('/') {
        let coin = coin.trim().to_uppercase();
        let quote = quote.trim().to_uppercase();
        if !coin.is_empty() && !quote.is_empty() {
            return Some((coin, quote));
        }
        return None;
    }

    if let Some((coin, quote)) = t.split_once('_') {
        let coin = coin.trim().to_uppercase();
        let quote = quote.trim().to_uppercase();
        if !coin.is_empty() && !quote.is_empty() {
            return Some((coin, quote));
        }
        return None;
    }

    let upper = t.to_uppercase();
    for quote in KNOWN_QUOTES {
        if upper.ends_with(quote) && upper.len() > quote.len() {
            let coin = upper[..upper.len() - quote.len()].to_string();
            return Some((coin, (*quote).to_string()));
        }
    }

    None
}

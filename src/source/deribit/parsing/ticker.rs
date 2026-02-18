//! Parse Deribit ticker subscription notification into OptionsTicker.

use crate::proto::{market_data_message, OptionsTicker};
use serde_json::Value;

/// Parse ticker notification `data` object.
/// Channel format: ticker.{instrument_name}.100ms or ticker.BTC-OPTION.100ms
pub fn parse_ticker(channel: &str, data: &Value) -> Option<market_data_message::Data> {
    let symbol = instrument_from_channel(channel, "ticker")?;
    let timestamp = data.get("timestamp")?.as_i64().or_else(|| {
        data.get("stats")
            .and_then(|s| s.get("timestamp"))
            .and_then(|t| t.as_i64())
    })?;
    let open_interest = data.get("open_interest").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mark_iv = data.get("mark_iv").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let best_bid_price = data.get("best_bid_price").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let best_ask_price = data.get("best_ask_price").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mark_price = data.get("mark_price").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let underlying_price = data
        .get("index_price")
        .and_then(|v| v.as_f64())
        .or_else(|| data.get("underlying_price").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    let last_price = data
        .get("last_price")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let bid_iv = data.get("bid_iv").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let ask_iv = data.get("ask_iv").and_then(|v| v.as_f64()).unwrap_or(0.0);

    Some(market_data_message::Data::OptionsTicker(OptionsTicker {
        symbol,
        timestamp,
        open_interest,
        mark_iv,
        best_bid_price,
        best_ask_price,
        mark_price,
        underlying_price,
        last_price,
        bid_iv,
        ask_iv,
    }))
}

fn instrument_from_channel(channel: &str, prefix: &str) -> Option<String> {
    // ticker.BTC-29MAR24-50000-C.100ms or ticker.BTC-OPTION.100ms
    let rest = channel.strip_prefix(&format!("{prefix}."))?;
    let before_interval = rest.rsplit_once('.').map(|(s, _)| s).unwrap_or(rest);
    Some(before_interval.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_from_ticker_channel() {
        assert_eq!(
            instrument_from_channel("ticker.BTC-29MAR24-50000-C.100ms", "ticker"),
            Some("BTC-29MAR24-50000-C".to_string())
        );
        assert_eq!(
            instrument_from_channel("ticker.BTC-OPTION.100ms", "ticker"),
            Some("BTC-OPTION".to_string())
        );
    }
}

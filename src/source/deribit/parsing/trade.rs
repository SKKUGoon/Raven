//! Parse Deribit trades subscription notification into Trade(s).

use crate::proto::{market_data_message, Trade};
use serde_json::Value;

/// Parse trades notification. `data` may be a single trade object or an array of trades.
pub fn parse_trades(channel: &str, data: &Value) -> Vec<market_data_message::Data> {
    let mut out = Vec::new();
    if let Some(arr) = data.as_array() {
        for item in arr {
            if let Some(t) = parse_single_trade(channel, item) {
                out.push(t);
            }
        }
    } else if let Some(t) = parse_single_trade(channel, data) {
        out.push(t);
    }
    out
}

fn parse_single_trade(channel: &str, data: &Value) -> Option<market_data_message::Data> {
    let symbol = data
        .get("instrument_name")
        .or_else(|| data.get("instrument"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| instrument_from_channel(channel, "trades"))?;
    let timestamp = data
        .get("timestamp")
        .or_else(|| data.get("time_stamp"))
        .and_then(|v| v.as_i64())?;
    let price = data.get("price").and_then(|v| v.as_f64())?;
    let quantity = data
        .get("amount")
        .or_else(|| data.get("quantity"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let direction = data
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let trade_id = data
        .get("trade_id")
        .or_else(|| data.get("tradeId"))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .map(|u| u.to_string())
        .unwrap_or_default();

    Some(market_data_message::Data::Trade(Trade {
        symbol,
        timestamp,
        price,
        quantity,
        side: direction.to_lowercase(),
        trade_id,
    }))
}

fn instrument_from_channel(channel: &str, prefix: &str) -> Option<String> {
    let rest = channel.strip_prefix(&format!("{prefix}."))?;
    let before_interval = rest.rsplit_once('.').map(|(s, _)| s).unwrap_or(rest);
    Some(before_interval.to_string())
}

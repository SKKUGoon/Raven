//! Parse Deribit price index subscription notification into PriceIndex.

use crate::proto::{market_data_message, PriceIndex};
use serde_json::Value;

/// Channel format: deribit_price_index.btc_usd
pub fn parse_price_index(channel: &str, data: &Value) -> Option<market_data_message::Data> {
    let index_name = channel
        .strip_prefix("deribit_price_index.")
        .unwrap_or(channel)
        .to_string();
    let price = data.get("price").and_then(|v| v.as_f64())?;
    let timestamp = data.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);

    Some(market_data_message::Data::PriceIndex(PriceIndex {
        index_name,
        price,
        timestamp,
    }))
}

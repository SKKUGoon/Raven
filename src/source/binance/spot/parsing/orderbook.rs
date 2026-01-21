use crate::proto::{market_data_message, OrderBookSnapshot, PriceLevel};
use serde_json::Value;

pub(crate) fn parse_binance_spot_orderbook(
    json: &str,
    symbol: &str,
) -> Option<market_data_message::Data> {
    let v: Value = serde_json::from_str(json).ok()?;

    // {
    //   "lastUpdateId": 160,  // Last update ID
    //   "bids": [ [ "0.0024", "10" ] ],
    //   "asks": [ [ "0.0026", "100" ] ]
    // }

    let sequence = v.get("lastUpdateId")?.as_i64().unwrap_or(0);
    // Spot Partial Depth doesn't have a specific timestamp field in the payload,
    // so we use the current system time as an approximation or 0.
    // However, usually streams might be wrapped if we used combined streams, but here we use raw.
    // Let's use current time since the payload doesn't provide it.
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let parse_levels = |levels: &Value| -> Option<Vec<PriceLevel>> {
        let arr = levels.as_array()?;
        let mut price_levels = Vec::with_capacity(arr.len());
        for item in arr {
            let item_arr = item.as_array()?;
            if item_arr.len() < 2 {
                continue;
            }
            let price = item_arr[0].as_str()?.parse().ok()?;
            let quantity = item_arr[1].as_str()?.parse().ok()?;
            price_levels.push(PriceLevel { price, quantity });
        }
        Some(price_levels)
    };

    let bids = parse_levels(v.get("bids")?)?;
    let asks = parse_levels(v.get("asks")?)?;

    Some(market_data_message::Data::Orderbook(OrderBookSnapshot {
        symbol: symbol.to_string(),
        timestamp,
        bids,
        asks,
        sequence,
    }))
}

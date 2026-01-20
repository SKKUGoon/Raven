use crate::proto::{market_data_message, OrderBookSnapshot, PriceLevel};
use serde_json::Value;

pub(crate) fn parse_binance_futures_orderbook(
    json: &str,
    symbol: &str,
) -> Option<market_data_message::Data> {
    let v: Value = serde_json::from_str(json).ok()?;

    // {"e":"depthUpdate","E":1571889248277,"T":1571889248276,"s":"BTCUSDT","U":390497796,"u":390497878,"pu":390497794,"b":[...],"a":[...]}
    // Note: Partial depth streams usually also use "depthUpdate" event type in Futures, or simply have the structure.
    // If it's a partial stream (snapshot), we treat it as a snapshot.
    if let Some(e) = v.get("e") {
        if e.as_str()? != "depthUpdate" {
            // Some partial streams might not have "e" or have different "e".
            // But if it has "b" and "a", we can try to parse.
        }
    }

    let timestamp = v.get("T")?.as_i64()?; // Transaction time usually better than Event time E
    let sequence = v.get("u")?.as_i64().unwrap_or(0); // Final update ID, useful for sequence

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

    let bids = parse_levels(v.get("b")?)?;
    let asks = parse_levels(v.get("a")?)?;

    Some(market_data_message::Data::Orderbook(OrderBookSnapshot {
        symbol: symbol.to_string(),
        timestamp,
        bids,
        asks,
        sequence,
    }))
}

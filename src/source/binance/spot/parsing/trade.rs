use crate::proto::{market_data_message, Trade};
use serde_json::Value;

pub(crate) fn parse_binance_trade(json: &str, symbol: &str) -> Option<market_data_message::Data> {
    let v: Value = serde_json::from_str(json).ok()?;

    // {"e":"trade","E":123456789,"s":"BNBBTC","t":12345,"p":"0.001","q":"100","b":88,"a":50,"T":123456785,"m":true,"M":true}
    if v.get("e")?.as_str()? != "trade" {
        return None;
    }

    let price = v.get("p")?.as_str()?.parse().ok()?;
    let quantity = v.get("q")?.as_str()?.parse().ok()?;
    let timestamp = v.get("T")?.as_i64()?;
    let trade_id = v.get("t")?.as_u64()?.to_string();
    let is_buyer_maker = v.get("m")?.as_bool()?; // true = sell (maker is buyer), false = buy (maker is seller) -> taker is buyer

    let side = if is_buyer_maker {
        "sell".to_string()
    } else {
        "buy".to_string()
    };

    Some(market_data_message::Data::Trade(Trade {
        symbol: symbol.to_string(),
        timestamp,
        price,
        quantity,
        side,
        trade_id,
    }))
}

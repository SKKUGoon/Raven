use crate::proto::{market_data_message, Candle};
use serde_json::Value;

/// Parses Binance Futures `kline` websocket events and emits a `Candle` **only when the kline is closed** (`k.x = true`).
///
/// Expected shape (trimmed):
/// {"e":"kline","E":...,"s":"BTCUSDT","k":{"t":...,"T":...,"s":"BTCUSDT","i":"1m","o":"...","c":"...","h":"...","l":"...","v":"...","x":true,...}}
///
/// Notes:
/// - Uses the embedded symbol (`k.s` preferred, fallback to top-level `s`).
/// - Uses the **kline close time** (`k.T`) as `Candle.timestamp` (milliseconds).
/// - Emits `interval` as `kline_<interval>` (e.g. `kline_1m`) to keep it distinct from Raven-derived bars.
pub(crate) fn parse_binance_futures_candle(
    json: &str,
    _symbol: &str,
) -> Option<market_data_message::Data> {
    let v: Value = serde_json::from_str(json).ok()?;

    if v.get("e")?.as_str()? != "kline" {
        return None;
    }

    let k = v.get("k")?;
    let is_closed = k.get("x")?.as_bool()?;
    if !is_closed {
        return None;
    }

    let symbol = k
        .get("s")
        .and_then(|s| s.as_str())
        .or_else(|| v.get("s").and_then(|s| s.as_str()))?
        .to_string();

    let interval = k.get("i")?.as_str()?.to_string();
    let open = k.get("o")?.as_str()?.parse().ok()?;
    let close = k.get("c")?.as_str()?.parse().ok()?;
    let high = k.get("h")?.as_str()?.parse().ok()?;
    let low = k.get("l")?.as_str()?.parse().ok()?;
    let volume = k.get("v")?.as_str()?.parse().ok()?;
    let timestamp = k.get("T")?.as_i64()?; // close time in ms

    Some(market_data_message::Data::Candle(Candle {
        symbol,
        timestamp,
        open,
        high,
        low,
        close,
        volume,
        interval: format!("kline_{interval}"),
        buy_ticks: 0,
        sell_ticks: 0,
        total_ticks: 0,
        theta: 0.0,
    }))
}

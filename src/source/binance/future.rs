use super::BinanceWsClient;
use crate::proto::{market_data_message, MarketDataMessage, OrderBookSnapshot, PriceLevel, Trade};
use crate::service::{StreamDataType, StreamKey, StreamManager, StreamWorker};
use crate::telemetry::{BINANCE_FUTURES_CONNECTIONS, BINANCE_FUTURES_TRADES};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::Status;

#[derive(Clone)]
pub struct BinanceFuturesWorker {
    client: BinanceWsClient,
}

#[tonic::async_trait]
impl StreamWorker for BinanceFuturesWorker {
    async fn run(&self, key: StreamKey, tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        let (symbol, stream_type, parser) = if key.data_type == StreamDataType::Orderbook {
            (
                key.symbol,
                "depth20@100ms".to_string(),
                parse_binance_futures_orderbook
                    as fn(&str, &str) -> Option<market_data_message::Data>,
            )
        } else {
            (
                key.symbol,
                "aggTrade".to_string(),
                parse_binance_futures_trade as fn(&str, &str) -> Option<market_data_message::Data>,
            )
        };

        self.client
            .run(
                symbol,
                stream_type,
                parser,
                tx,
                &BINANCE_FUTURES_TRADES,
                &BINANCE_FUTURES_CONNECTIONS,
            )
            .await;
    }
}

pub type BinanceFuturesService = StreamManager<BinanceFuturesWorker>;

pub fn new() -> BinanceFuturesService {
    let client = BinanceWsClient::new(
        "wss://fstream.binance.com/ws/".to_string(),
        "binance_futures".to_string(),
        "BINANCE_FUTURES".to_string(),
    );

    let worker = BinanceFuturesWorker { client };
    StreamManager::new(Arc::new(worker), 10000, true)
}

fn parse_binance_futures_trade(json: &str, symbol: &str) -> Option<market_data_message::Data> {
    let v: Value = serde_json::from_str(json).ok()?;

    // {"e":"aggTrade","E":123456789,"s":"BTCUSDT","a":12345,"p":"0.001","q":"100","f":100,"l":105,"T":123456785,"m":true}
    if v.get("e")?.as_str()? != "aggTrade" {
        return None;
    }

    let price = v.get("p")?.as_str()?.parse().ok()?;
    let quantity = v.get("q")?.as_str()?.parse().ok()?;
    let timestamp = v.get("T")?.as_i64()?;
    let trade_id = v.get("a")?.as_u64()?.to_string(); // Aggregated trade ID
    let is_buyer_maker = v.get("m")?.as_bool()?; // true = sell, false = buy

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

fn parse_binance_futures_orderbook(json: &str, symbol: &str) -> Option<market_data_message::Data> {
    let v: Value = serde_json::from_str(json).ok()?;

    // {"e":"depthUpdate","E":1571889248277,"T":1571889248276,"s":"BTCUSDT","U":390497796,"u":390497878,"pu":390497794,"b":[...],"a":[...]}
    // Note: Partial depth streams usually also use "depthUpdate" event type in Futures, or simply have the structure.
    // If it's a partial stream (snapshot), we treat it as a snapshot.
    
    // For partial depth, the event type is typically "depthUpdate".
    // We should check the event type or just check for existence of "b" and "a".
    // But to be safe, let's check "e".
    if let Some(e) = v.get("e") {
        if e.as_str()? != "depthUpdate" {
            // Some partial streams might not have "e" or have different "e".
            // But if it has "b" and "a", we can try to parse.
            // Let's rely on "b" and "a" being present.
        }
    }

    let timestamp = v.get("T")?.as_i64()?; // Transaction time usually better than Event time E
    let sequence = v.get("u")?.as_i64().unwrap_or(0); // Final update ID, useful for sequence

    let parse_levels = |levels: &Value| -> Option<Vec<PriceLevel>> {
        let arr = levels.as_array()?;
        let mut price_levels = Vec::with_capacity(arr.len());
        for item in arr {
            let item_arr = item.as_array()?;
            if item_arr.len() < 2 { continue; }
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

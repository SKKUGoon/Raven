use crate::proto::{market_data_message, MarketDataMessage, OrderBookSnapshot, PriceLevel, Trade};
use crate::service::{StreamDataType, StreamKey, StreamManager, StreamWorker};
use super::BinanceWsClient;
use crate::telemetry::{BINANCE_SPOT_CONNECTIONS, BINANCE_SPOT_TRADES};
use serde_json::Value;
use tokio::sync::broadcast;
use tonic::Status;
use std::sync::Arc;

#[derive(Clone)]
pub struct BinanceSpotWorker {
    client: BinanceWsClient,
}

#[tonic::async_trait]
impl StreamWorker for BinanceSpotWorker {
    async fn run(&self, key: StreamKey, tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        let (symbol, stream_type, parser) = if key.data_type == StreamDataType::Orderbook {
            (
                key.symbol,
                "depth20@100ms".to_string(),
                parse_binance_spot_orderbook as fn(&str, &str) -> Option<market_data_message::Data>,
            )
        } else {
            (
                key.symbol,
                "trade".to_string(),
                parse_binance_trade as fn(&str, &str) -> Option<market_data_message::Data>,
            )
        };

        self.client
            .run(
                symbol,
                stream_type,
                parser,
                tx,
                &BINANCE_SPOT_TRADES,
                &BINANCE_SPOT_CONNECTIONS,
            )
            .await;
    }
}

pub type BinanceSpotService = StreamManager<BinanceSpotWorker>;

pub fn new() -> BinanceSpotService {
    let client = BinanceWsClient::new(
        "wss://stream.binance.com:9443/ws/".to_string(),
        "binance_spot".to_string(),
        "BINANCE_SPOT".to_string(),
    );

    let worker = BinanceSpotWorker { client };
    StreamManager::new(Arc::new(worker), 10000, true)
}

fn parse_binance_trade(json: &str, symbol: &str) -> Option<market_data_message::Data> {
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

fn parse_binance_spot_orderbook(json: &str, symbol: &str) -> Option<market_data_message::Data> {
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
            if item_arr.len() < 2 { continue; }
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

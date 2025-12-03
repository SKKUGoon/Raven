use crate::proto::{MarketDataMessage, Trade};
use crate::service::{StreamManager, StreamWorker};
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
    async fn run(&self, symbol: String, tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        self.client
            .run(symbol, tx, &BINANCE_SPOT_TRADES, &BINANCE_SPOT_CONNECTIONS)
            .await;
    }
}

pub type BinanceSpotService = StreamManager<BinanceSpotWorker>;

pub fn new() -> BinanceSpotService {
    let client = BinanceWsClient::new(
        "wss://stream.binance.com:9443/ws/".to_string(),
        "trade".to_string(),
        "binance_spot".to_string(),
        parse_binance_trade,
    );

    let worker = BinanceSpotWorker { client };
    StreamManager::new(Arc::new(worker), 10000, true)
}

fn parse_binance_trade(json: &str, symbol: &str) -> Option<Trade> {
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

    Some(Trade {
        symbol: symbol.to_string(),
        timestamp,
        price,
        quantity,
        side,
        trade_id,
    })
}

use crate::proto::{MarketDataMessage, Trade};
use crate::service::StreamManager;
use super::BinanceWsClient;
use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, register_int_gauge, IntCounterVec, IntGauge};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::broadcast;
use tonic::Status;

lazy_static! {
    static ref TRADES_PROCESSED: IntCounterVec = register_int_counter_vec!(
        "raven_binance_spot_trades_processed_total",
        "Total number of trades processed",
        &["symbol"]
    )
    .unwrap();
    static ref ACTIVE_CONNECTIONS: IntGauge = register_int_gauge!(
        "raven_binance_spot_active_connections",
        "Number of active WebSocket connections"
    )
    .unwrap();
}

pub type BinanceSpotService = StreamManager<
    Box<
        dyn Fn(
                String,
                broadcast::Sender<Result<MarketDataMessage, Status>>,
            ) -> Pin<Box<dyn Future<Output = ()> + Send>>
            + Send
            + Sync,
    >,
>;

pub fn new() -> BinanceSpotService {
    let client = BinanceWsClient::new(
        "wss://stream.binance.com:9443/ws/".to_string(),
        "trade".to_string(),
        "binance_spot".to_string(),
        parse_binance_trade,
    );

    StreamManager::new(Box::new(move |symbol, tx| {
        let client = client.clone();
        Box::pin(async move {
            client.run(symbol, tx, &TRADES_PROCESSED, &ACTIVE_CONNECTIONS).await;
        })
    }))
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


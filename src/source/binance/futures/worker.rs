use super::parsing::{orderbook::parse_binance_futures_orderbook, trade::parse_binance_futures_trade};
use super::super::BinanceWsClient;
use crate::proto::{market_data_message, MarketDataMessage};
use crate::service::{StreamDataType, StreamKey, StreamManager, StreamWorker};
use crate::telemetry::{BINANCE_FUTURES_CONNECTIONS, BINANCE_FUTURES_TRADES};
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
                parse_binance_futures_orderbook as fn(&str, &str) -> Option<market_data_message::Data>,
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

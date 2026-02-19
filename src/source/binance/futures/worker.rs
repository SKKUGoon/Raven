use super::super::constants::{
    BINANCE_FUTURES_STREAM_AGG_TRADE, BINANCE_FUTURES_WS_URL,
    BINANCE_STREAM_ORDERBOOK_DEPTH20_100MS, VENUE_BINANCE_FUTURES,
};
use super::super::BinanceWsClient;
use super::parsing::{
    orderbook::parse_binance_futures_orderbook, trade::parse_binance_futures_trade,
};
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
                BINANCE_STREAM_ORDERBOOK_DEPTH20_100MS.to_string(),
                parse_binance_futures_orderbook
                    as fn(&str, &str) -> Option<market_data_message::Data>,
            )
        } else {
            (
                key.symbol,
                BINANCE_FUTURES_STREAM_AGG_TRADE.to_string(),
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
        BINANCE_FUTURES_WS_URL.to_string(),
        "binance_futures".to_string(),
        VENUE_BINANCE_FUTURES.to_string(),
    );

    let worker = BinanceFuturesWorker { client };
    StreamManager::new(Arc::new(worker), 10000, true)
}

use super::parsing::{orderbook::parse_binance_spot_orderbook, trade::parse_binance_trade};
use super::super::BinanceWsClient;
use super::super::constants::{
    BINANCE_SPOT_STREAM_TRADE, BINANCE_SPOT_WS_URL, BINANCE_STREAM_ORDERBOOK_DEPTH20_100MS,
    VENUE_BINANCE_SPOT,
};
use crate::proto::{market_data_message, MarketDataMessage};
use crate::service::{StreamDataType, StreamKey, StreamManager, StreamWorker};
use crate::telemetry::{BINANCE_SPOT_CONNECTIONS, BINANCE_SPOT_TRADES};
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::Status;

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
                BINANCE_STREAM_ORDERBOOK_DEPTH20_100MS.to_string(),
                parse_binance_spot_orderbook as fn(&str, &str) -> Option<market_data_message::Data>,
            )
        } else {
            (
                key.symbol,
                BINANCE_SPOT_STREAM_TRADE.to_string(),
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
        BINANCE_SPOT_WS_URL.to_string(),
        "binance_spot".to_string(),
        VENUE_BINANCE_SPOT.to_string(),
    );

    let worker = BinanceSpotWorker { client };
    StreamManager::new(Arc::new(worker), 10000, true)
}

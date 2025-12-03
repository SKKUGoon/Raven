use crate::proto::market_data_client::MarketDataClient;
use crate::proto::market_data_message;
use crate::proto::{Candle, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamManager, StreamWorker};
use crate::telemetry::{BARS_ACTIVE_AGGREGATIONS, BARS_GENERATED};
use tokio::sync::broadcast;
use tonic::{Request, Status};
use tracing::{error, info};
use std::sync::Arc;

#[derive(Clone, Debug)]
struct Bar {
    symbol: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    start_time: i64,
}

impl Bar {
    fn new(symbol: String, price: f64, quantity: f64, timestamp: i64) -> Self {
        Self {
            symbol,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: quantity,
            start_time: timestamp - (timestamp % 60000), // Snap to minute
        }
    }

    fn update(&mut self, price: f64, quantity: f64, timestamp: i64) -> Option<Bar> {
        let period_start = timestamp - (timestamp % 60000);

        if period_start > self.start_time {
            // New bar started, return completed bar
            let closed_bar = self.clone();

            // Reset current bar
            self.start_time = period_start;
            self.open = price;
            self.high = price;
            self.low = price;
            self.close = price;
            self.volume = quantity;

            Some(closed_bar)
        } else {
            // Update current bar
            self.high = f64::max(self.high, price);
            self.low = f64::min(self.low, price);
            self.close = price;
            self.volume += quantity;
            None
        }
    }

    fn to_proto(&self) -> Candle {
        Candle {
            symbol: self.symbol.clone(),
            timestamp: self.start_time,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
            interval: "1m".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct BarsWorker {
    upstream_url: String,
}

#[tonic::async_trait]
impl StreamWorker for BarsWorker {
    async fn run(&self, symbol: String, tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        run_aggregation(self.upstream_url.clone(), symbol, tx).await;
    }
}

pub type BarService = StreamManager<BarsWorker>;

pub fn new(upstream_url: String) -> BarService {
    let worker = BarsWorker { upstream_url };
    StreamManager::new(Arc::new(worker), 100, true)
}

async fn run_aggregation(
    upstream_url: String,
    symbol: String,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
) {
    info!("Starting bar aggregation for {}", symbol);
    BARS_ACTIVE_AGGREGATIONS.inc();

    let mut client = match MarketDataClient::connect(upstream_url).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to upstream: {}", e);
            BARS_ACTIVE_AGGREGATIONS.dec();
            return;
        }
    };

    let request = Request::new(MarketDataRequest {
        symbol: symbol.clone(),
        data_type: DataType::Trade as i32,
    });

    let mut stream = match client.subscribe(request).await {
        Ok(res) => res.into_inner(),
        Err(e) => {
            error!("Failed to subscribe to trades: {}", e);
            BARS_ACTIVE_AGGREGATIONS.dec();
            return;
        }
    };

    let mut current_bar: Option<Bar> = None;

    while let Ok(Some(msg)) = stream.message().await {
        if let Some(market_data_message::Data::Trade(trade)) = msg.data {
            if let Some(bar) = &mut current_bar {
                if let Some(completed) = bar.update(trade.price, trade.quantity, trade.timestamp) {
                    // Emit completed bar
                    BARS_GENERATED.with_label_values(&[&symbol]).inc();
                    let candle_msg = MarketDataMessage {
                        exchange: "raven_bars".to_string(),
                        data: Some(market_data_message::Data::Candle(completed.to_proto())),
                    };
                    if tx.send(Ok(candle_msg)).is_err() {
                        break;
                    }
                }
            } else {
                current_bar = Some(Bar::new(
                    symbol.clone(),
                    trade.price,
                    trade.quantity,
                    trade.timestamp,
                ));
            }
        }
    }
    info!("Aggregation task ended for {}", symbol);
    BARS_ACTIVE_AGGREGATIONS.dec();
}

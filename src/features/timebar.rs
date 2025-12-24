use crate::proto::market_data_client::MarketDataClient;
use crate::proto::market_data_message;
use crate::proto::{Candle, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamKey, StreamManager, StreamWorker};
use crate::telemetry::{TIMEBAR_MINUTES_ACTIVE_AGGREGATIONS, TIMEBAR_MINUTES_GENERATED};
use crate::utils::retry::retry_forever;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::{Request, Status};
use tracing::{error, info, warn};

#[derive(Clone, Debug)]
struct TimeBar {
    symbol: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    start_time: i64,
    buy_ticks: u64,
    sell_ticks: u64,
    total_ticks: u64,
    interval_seconds: u64,
}

impl TimeBar {
    fn new(
        symbol: String,
        price: f64,
        quantity: f64,
        timestamp: i64,
        side: &str,
        interval_seconds: u64,
    ) -> Self {
        let (buy_ticks, sell_ticks) = if side == "buy" { (1, 0) } else { (0, 1) };
        let interval_ms = (interval_seconds * 1000) as i64;
        Self {
            symbol,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: quantity,
            start_time: timestamp - (timestamp % interval_ms),
            buy_ticks,
            sell_ticks,
            total_ticks: 1,
            interval_seconds,
        }
    }

    fn update(
        &mut self,
        price: f64,
        quantity: f64,
        timestamp: i64,
        side: &str,
    ) -> Option<TimeBar> {
        let interval_ms = (self.interval_seconds * 1000) as i64;
        let period_start = timestamp - (timestamp % interval_ms);
        let (is_buy, is_sell) = if side == "buy" { (1, 0) } else { (0, 1) };

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
            self.buy_ticks = is_buy;
            self.sell_ticks = is_sell;
            self.total_ticks = 1;

            Some(closed_bar)
        } else {
            // Update current bar
            self.high = f64::max(self.high, price);
            self.low = f64::min(self.low, price);
            self.close = price;
            self.volume += quantity;
            self.buy_ticks += is_buy;
            self.sell_ticks += is_sell;
            self.total_ticks += 1;
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
            interval: format!("{}s", self.interval_seconds),
            buy_ticks: self.buy_ticks,
            sell_ticks: self.sell_ticks,
            total_ticks: self.total_ticks,
            theta: 0.0,
        }
    }
}

#[derive(Clone)]
pub struct TimeBarWorker {
    upstreams: HashMap<String, String>,
    interval_seconds: u64,
}

#[tonic::async_trait]
impl StreamWorker for TimeBarWorker {
    async fn run(
        &self,
        key: StreamKey,
        tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
    ) {
        let symbol = key.symbol.clone();
        let venue = key.venue.clone().unwrap_or_else(|| "BINANCE_SPOT".to_string());

        // Try to find the upstream URL for the requested exchange, or fall back to BINANCE_SPOT
        let upstream_url = if let Some(url) = self.upstreams.get(&venue) {
            url.clone()
        } else {
            warn!(
                "No upstream configured for exchange '{}'. Defaulting to BINANCE_SPOT if available.",
                venue
            );
            if let Some(url) = self.upstreams.get("BINANCE_SPOT") {
                url.clone()
            } else {
                error!(
                    "No upstream configuration found for BINANCE_SPOT. Cannot start aggregation for {}",
                    key
                );
                return;
            }
        };

        run_aggregation(
            upstream_url,
            symbol,
            key.to_string(),
            venue,
            tx,
            self.interval_seconds,
        )
        .await;
    }
}

pub type TimeBarService = StreamManager<TimeBarWorker>;

pub fn new(upstreams: HashMap<String, String>, interval_seconds: u64) -> TimeBarService {
    let worker = TimeBarWorker {
        upstreams,
        interval_seconds,
    };
    StreamManager::new(Arc::new(worker), 100, true)
}

async fn run_aggregation(
    upstream_url: String,
    subscription_symbol: String,
    output_symbol: String,
    venue: String,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
    interval_seconds: u64,
) {
    info!(
        "Starting timebar aggregation ({}s) for {} using upstream {}",
        interval_seconds, output_symbol, upstream_url
    );
    TIMEBAR_MINUTES_ACTIVE_AGGREGATIONS.inc();

    let mut current_bar: Option<TimeBar> = None;

    loop {
        let mut stream = retry_forever(
            &format!("timebar connect+subscribe ({output_symbol})"),
            std::time::Duration::from_secs(2),
            || async {
                let mut client = MarketDataClient::connect(upstream_url.clone())
                    .await
                    .map_err(|e| tonic::Status::unavailable(e.to_string()))?;
                let request = Request::new(MarketDataRequest {
                    symbol: subscription_symbol.clone(),
                    data_type: DataType::Trade as i32,
                    venue: venue.clone(),
                });
                let res = client.subscribe(request).await?;
                Ok::<_, tonic::Status>(res.into_inner())
            },
        )
        .await;

        loop {
            match stream.message().await {
                Ok(Some(msg)) => {
                    if let Some(market_data_message::Data::Trade(trade)) = msg.data {
                        if let Some(bar) = &mut current_bar {
                            if let Some(completed) = bar.update(
                                trade.price,
                                trade.quantity,
                                trade.timestamp,
                                &trade.side,
                            ) {
                                // Emit completed bar
                                TIMEBAR_MINUTES_GENERATED
                                    .with_label_values(&[&output_symbol])
                                    .inc();
                                let candle_msg = MarketDataMessage {
                                    // Backwards-compat: keep exchange as producer for older consumers.
                                    exchange: "raven_timebar".to_string(),
                                    venue: venue.clone(),
                                    producer: "raven_timebar".to_string(),
                                    data: Some(market_data_message::Data::Candle(
                                        completed.to_proto(),
                                    )),
                                };
                                if tx.send(Ok(candle_msg)).is_err() {
                                    info!("No subscribers for {output_symbol}; ending aggregation task.");
                                    info!("Aggregation task ended for {}", output_symbol);
                                    TIMEBAR_MINUTES_ACTIVE_AGGREGATIONS.dec();
                                    return;
                                }
                            }
                        } else {
                            current_bar = Some(TimeBar::new(
                                subscription_symbol.clone(),
                                trade.price,
                                trade.quantity,
                                trade.timestamp,
                                &trade.side,
                                interval_seconds,
                            ));
                        }
                    }
                }
                Ok(None) => {
                    warn!("Upstream stream ended for {output_symbol}. Reconnecting in 2s...");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    break;
                }
                Err(e) => {
                    warn!("Upstream stream error for {output_symbol}: {e}. Reconnecting in 2s...");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    break;
                }
            }
        }
    }
}

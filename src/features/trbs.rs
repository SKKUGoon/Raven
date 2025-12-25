use crate::config::TibsConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::market_data_message;
use crate::proto::{Candle, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamKey, StreamManager, StreamWorker};
use crate::telemetry::{TRBS_ACTIVE_AGGREGATIONS, TRBS_GENERATED};
use crate::utils::retry::retry_forever;
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::{Request, Status};
use tracing::{error, info, warn};

/// Direction of ticks (run sign)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickDirection {
    Buy,
    Sell,
}

impl TickDirection {
    #[inline]
    pub fn is_buy(&self) -> bool {
        matches!(self, TickDirection::Buy)
    }
}

/// Tick Run Bar (TRB), similar to the provided Python implementation:
/// - Tracks a `current_run` length and `run_sign` (buy/sell)
/// - Tracks `theta` as the best (max) run length observed so far in the bar
/// - Closes when `theta > threshold`, where threshold is `E[T] * max(p, 1-p)`
#[derive(Debug, Clone)]
pub struct TickRunsBar {
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub trade_size: f64,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub tick_size: u32,
    pub buy_ticks: u64,
    pub sell_ticks: u64,
    pub theta: u32,
    pub current_run: u32,
    pub run_sign: TickDirection,
    pub bar_open: bool,
}

impl TickRunsBar {
    pub fn new(
        symbol: String,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Self {
        let buy_ticks = if b_t.is_buy() { 1 } else { 0 };
        let sell_ticks = if b_t.is_buy() { 0 } else { 1 };
        Self {
            symbol,
            open: price,
            high: price,
            low: price,
            close: price,
            trade_size: volume,
            open_time: ts,
            close_time: ts,
            tick_size: 1,
            buy_ticks,
            sell_ticks,
            theta: 1,
            current_run: 1,
            run_sign: b_t,
            bar_open: true,
        }
    }

    fn close(&mut self) {
        self.bar_open = false;
    }

    fn probability_buy(&self) -> f64 {
        if self.tick_size == 0 {
            return 0.0;
        }
        self.buy_ticks as f64 / self.tick_size as f64
    }

    fn update_current_run(&mut self, b_t: TickDirection) {
        if b_t == self.run_sign {
            self.current_run += 1;
        } else {
            // Mirror the provided Python: theta updates on run reversal, and current_run resets to 1.
            self.theta = self.theta.max(self.current_run);
            self.run_sign = b_t;
            self.current_run = 1;
        }
    }

    fn update(&mut self, price: f64, volume: f64, b_t: TickDirection, ts: DateTime<Utc>) {
        if !self.bar_open {
            return;
        }

        // Update the momentum of the run (before updating OHLCV), like the Python version.
        self.update_current_run(b_t);

        // Update bar OHLCV
        self.high = price.max(self.high);
        self.low = price.min(self.low);
        self.close = price;
        self.trade_size += volume;
        self.close_time = ts;

        // Tick bookkeeping
        self.tick_size += 1;
        if b_t.is_buy() {
            self.buy_ticks += 1;
        } else {
            self.sell_ticks += 1;
        }
    }

    fn to_proto_with_interval(&self, interval: String) -> Candle {
        Candle {
            symbol: self.symbol.clone(),
            timestamp: self.open_time.timestamp_millis(),
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.trade_size,
            interval,
            buy_ticks: self.buy_ticks,
            sell_ticks: self.sell_ticks,
            total_ticks: self.tick_size as u64,
            theta: self.theta as f64,
        }
    }
}

#[derive(Clone)]
pub struct TickRunsState {
    pub threshold: f64,
    pub size_ewma: f64,
    pub prob_ewma: f64,
    pub alpha_size: f64,
    pub alpha_prob: f64,
    pub size_boundary: (f64, f64),
    pub current_bar: Option<TickRunsBar>,
}

impl TickRunsState {
    pub fn new(
        initial_size: f64,
        initial_p_buy: f64,
        alpha_size: f64,
        alpha_prob: f64,
        size_boundary: (f64, f64),
    ) -> Self {
        let size_ewma = initial_size;
        let prob_ewma = initial_p_buy;
        let threshold = size_ewma * prob_ewma.max(1.0 - prob_ewma);
        Self {
            threshold,
            size_ewma,
            prob_ewma,
            alpha_size,
            alpha_prob,
            size_boundary,
            current_bar: None,
        }
    }

    fn bound(val: f64, min: f64, max: f64) -> f64 {
        val.max(min).min(max)
    }

    fn on_tick(
        &mut self,
        symbol: String,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Option<TickRunsBar> {
        if self.current_bar.is_none() {
            self.current_bar = Some(TickRunsBar::new(symbol, price, volume, b_t, ts));
            return None;
        }

        let bar = self.current_bar.as_mut().unwrap();
        bar.update(price, volume, b_t, ts);

        if (bar.theta as f64) > self.threshold {
            bar.close();
            let closed = self.current_bar.take().unwrap();

            // Update EWMAs (same structure as Python TRB)
            let new_prob = closed.probability_buy();
            self.prob_ewma = self.prob_ewma * (1.0 - self.alpha_prob) + new_prob * self.alpha_prob;

            let new_size = closed.tick_size as f64;
            self.size_ewma = self.size_ewma * (1.0 - self.alpha_size) + new_size * self.alpha_size;
            self.size_ewma =
                Self::bound(self.size_ewma, self.size_boundary.0, self.size_boundary.1);

            self.threshold = self.size_ewma * self.prob_ewma.max(1.0 - self.prob_ewma);
            return Some(closed);
        }

        None
    }
}

fn bounds_from_config(config: &TibsConfig, initial_size: f64) -> (f64, f64) {
    // Preferred: percentage bounds relative to initial_size
    if let (Some(min_pct), Some(max_pct)) = (config.size_min_pct, config.size_max_pct) {
        let min = initial_size * (1.0 - min_pct);
        let max = initial_size * (1.0 + max_pct);
        return (min, max);
    }
    // Back-compat: absolute bounds
    if let (Some(min), Some(max)) = (config.size_min, config.size_max) {
        return (min, max);
    }
    // Sensible default: +/- 10%
    (initial_size * 0.9, initial_size * 1.1)
}

#[derive(Clone)]
pub struct TrbsWorker {
    upstreams: HashMap<String, String>,
    // Reuse TibsConfig fields: `alpha_imbl` is treated as probability EWMA alpha for TRBs.
    config: TibsConfig,
    interval: String,
}

#[tonic::async_trait]
impl StreamWorker for TrbsWorker {
    async fn run(&self, key: StreamKey, tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        let symbol = key.symbol.clone();
        let venue = key
            .venue
            .clone()
            .unwrap_or_else(|| "BINANCE_SPOT".to_string());
        let interval = self.interval.clone();

        // Try to find the upstream URL for the requested exchange, or fall back to BINANCE_SPOT
        let upstream_url = if let Some(url) = self.upstreams.get(&venue) {
            url.clone()
        } else {
            warn!("No upstream configured for exchange '{}'. Defaulting to BINANCE_SPOT if available.", venue);
            if let Some(url) = self.upstreams.get("BINANCE_SPOT") {
                url.clone()
            } else {
                error!("No upstream configuration found for BINANCE_SPOT. Cannot start aggregation for {}", key);
                return;
            }
        };

        run_trb_aggregation(
            upstream_url,
            self.config.clone(),
            symbol,
            key.to_string(),
            venue,
            tx,
            interval,
        )
        .await;
    }
}

pub type TrbsService = StreamManager<TrbsWorker>;

pub fn new(
    upstreams: HashMap<String, String>,
    config: TibsConfig,
    interval: String,
) -> TrbsService {
    let worker = TrbsWorker {
        upstreams,
        config,
        interval,
    };
    StreamManager::new(Arc::new(worker), 100, true)
}

async fn run_trb_aggregation(
    upstream_url: String,
    config: TibsConfig,
    subscription_symbol: String,
    output_symbol: String,
    venue: String,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
    interval: String,
) {
    info!(
        "Starting TRB aggregation for {} using upstream {}",
        output_symbol, upstream_url
    );
    TRBS_ACTIVE_AGGREGATIONS.inc();

    // Initialize State with config
    let mut state = TickRunsState::new(
        config.initial_size,
        config.initial_p_buy,
        config.alpha_size,
        config.alpha_imbl, // note: reused as probability EWMA alpha
        bounds_from_config(&config, config.initial_size),
    );

    loop {
        let mut stream = retry_forever(
            &format!("trbs connect+subscribe ({output_symbol})"),
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
                        let direction = match trade.side.as_str() {
                            "buy" => TickDirection::Buy,
                            _ => TickDirection::Sell,
                        };
                        let ts = Utc.timestamp_millis_opt(trade.timestamp).unwrap();

                        if let Some(closed_bar) = state.on_tick(
                            subscription_symbol.clone(),
                            trade.price,
                            trade.quantity,
                            direction,
                            ts,
                        ) {
                            TRBS_GENERATED.with_label_values(&[&output_symbol]).inc();
                            let msg = MarketDataMessage {
                                // Backwards-compat: keep exchange as producer for older consumers.
                                exchange: "raven_trbs".to_string(),
                                venue: venue.clone(),
                                producer: "raven_trbs".to_string(),
                                data: Some(market_data_message::Data::Candle(
                                    closed_bar.to_proto_with_interval(interval.clone()),
                                )),
                            };
                            if tx.send(Ok(msg)).is_err() {
                                info!(
                                    "No subscribers for {output_symbol}; ending aggregation task."
                                );
                                info!("TRB aggregation task ended for {}", output_symbol);
                                TRBS_ACTIVE_AGGREGATIONS.dec();
                                return;
                            }
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

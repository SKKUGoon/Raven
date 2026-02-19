use crate::config::TibsConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::market_data_message;
use crate::proto::{Candle, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamKey, StreamManager, StreamWorker};
use crate::telemetry::{VIBS_ACTIVE_AGGREGATIONS, VIBS_GENERATED};
use crate::utils::retry::retry_forever;
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::{Request, Status};
use tracing::{error, info, warn};

/// Direction of the tick (aggressor side).
#[derive(Clone, Copy, Debug)]
pub enum TickDirection {
    Buy,
    Sell,
}

impl TickDirection {
    #[inline]
    pub fn sign(&self) -> f64 {
        match self {
            TickDirection::Buy => 1.0,
            TickDirection::Sell => -1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VolumeImbalanceBar {
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
    /// Cumulative signed volume imbalance: theta = sum(v_t * b_t)
    pub theta: f64,
    pub bar_open: bool,
}

impl VolumeImbalanceBar {
    pub fn new(
        symbol: String,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Self {
        let (buy_ticks, sell_ticks) = match b_t {
            TickDirection::Buy => (1, 0),
            TickDirection::Sell => (0, 1),
        };
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
            theta: volume * b_t.sign(),
            bar_open: true,
        }
    }

    fn update(&mut self, price: f64, volume: f64, b_t: TickDirection, ts: DateTime<Utc>) {
        if self.bar_open {
            self.high = price.max(self.high);
            self.low = price.min(self.low);
            self.close = price;
            self.trade_size += volume;
            self.close_time = ts;
            self.tick_size += 1;
            match b_t {
                TickDirection::Buy => self.buy_ticks += 1,
                TickDirection::Sell => self.sell_ticks += 1,
            }
            self.theta += volume * b_t.sign();
        }
    }

    fn close(&mut self) {
        self.bar_open = false;
    }

    fn p_buy(&self) -> f64 {
        if self.tick_size == 0 {
            return 0.5;
        }
        self.buy_ticks as f64 / self.tick_size as f64
    }

    fn avg_volume_per_tick(&self) -> f64 {
        if self.tick_size == 0 {
            return 0.0;
        }
        self.trade_size / self.tick_size as f64
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
            // For VIBS, we emit the signed volume imbalance in `theta`.
            theta: self.theta,
        }
    }
}

#[derive(Clone)]
pub struct VolumeImbalanceState {
    pub threshold: f64,
    pub size_ewma: f64,
    pub p_buy_ewma: f64,
    pub vol_ewma: f64,
    pub alpha_size: f64,
    pub alpha_imbl: f64,
    pub size_boundary: (f64, f64),
    pub current_bar: Option<VolumeImbalanceBar>,
}

impl VolumeImbalanceState {
    pub fn new(
        initial_size: f64,
        initial_p_buy: f64,
        alpha_size: f64,
        alpha_imbl: f64,
        size_boundary: (f64, f64),
    ) -> Self {
        let size_ewma = initial_size;
        let p_buy_ewma = initial_p_buy;
        // Initialize to 0 so we can seed from the first observed tick volume.
        let vol_ewma = 0.0;
        let threshold = 0.0;

        Self {
            threshold,
            size_ewma,
            p_buy_ewma,
            vol_ewma,
            alpha_size,
            alpha_imbl,
            size_boundary,
            current_bar: None,
        }
    }

    fn bound(val: f64, min: f64, max: f64) -> f64 {
        val.max(min).min(max)
    }

    fn update_threshold(&mut self) {
        // E[theta] ~= E[T] * E[v] * E[b]
        let e_b = 2.0 * self.p_buy_ewma - 1.0;
        self.threshold = self.size_ewma * self.vol_ewma * e_b;
    }

    fn on_tick(
        &mut self,
        symbol: String,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Option<VolumeImbalanceBar> {
        if self.current_bar.is_none() {
            // Seed E[v] from the first tick.
            if self.vol_ewma == 0.0 {
                self.vol_ewma = volume.abs().max(1e-12);
                self.update_threshold();
            }
            self.current_bar = Some(VolumeImbalanceBar::new(symbol, price, volume, b_t, ts));
            return None;
        }

        let bar = self.current_bar.as_mut().unwrap();
        bar.update(price, volume, b_t, ts);

        if bar.theta.abs() > self.threshold.abs() {
            bar.close();
            let closed = self.current_bar.take().unwrap();

            // Update E[b] from the realized buy probability in the closed bar.
            let new_p_buy = closed.p_buy();
            self.p_buy_ewma =
                self.alpha_imbl * new_p_buy + (1.0 - self.alpha_imbl) * self.p_buy_ewma;

            // Update E[v] from realized average volume per tick.
            let new_vol = closed.avg_volume_per_tick().abs().max(1e-12);
            self.vol_ewma = self.alpha_imbl * new_vol + (1.0 - self.alpha_imbl) * self.vol_ewma;

            // Update E[T] from realized ticks-per-bar (and clamp).
            let new_size = closed.tick_size as f64;
            self.size_ewma = self.alpha_size * new_size + (1.0 - self.alpha_size) * self.size_ewma;
            self.size_ewma =
                Self::bound(self.size_ewma, self.size_boundary.0, self.size_boundary.1);

            self.update_threshold();
            return Some(closed);
        }

        None
    }
}

#[derive(Clone)]
pub struct VibsWorker {
    upstreams: HashMap<String, String>,
    config: TibsConfig,
    interval: String,
}

#[tonic::async_trait]
impl StreamWorker for VibsWorker {
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

        run_vib_aggregation(
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

pub type VibsService = StreamManager<VibsWorker>;

pub fn new(
    upstreams: HashMap<String, String>,
    config: TibsConfig,
    interval: String,
) -> VibsService {
    let worker = VibsWorker {
        upstreams,
        config,
        interval,
    };
    StreamManager::new(Arc::new(worker), 100, true)
}

async fn run_vib_aggregation(
    upstream_url: String,
    config: TibsConfig,
    subscription_symbol: String,
    output_symbol: String,
    venue: String,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
    interval: String,
) {
    info!(
        "Starting VIB aggregation for {} using upstream {}",
        output_symbol, upstream_url
    );
    VIBS_ACTIVE_AGGREGATIONS.inc();

    let mut state = VolumeImbalanceState::new(
        config.initial_size,
        config.initial_p_buy,
        config.alpha_size,
        config.alpha_imbl,
        super::imbalance_bounds_from_config(&config, config.initial_size),
    );

    loop {
        let mut stream = retry_forever(
            &format!("vibs connect+subscribe ({output_symbol})"),
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
                            VIBS_GENERATED.with_label_values(&[&output_symbol]).inc();
                            let msg = MarketDataMessage {
                                venue: venue.clone(),
                                producer: "raven_vibs".to_string(),
                                data: Some(market_data_message::Data::Candle(
                                    closed_bar.to_proto_with_interval(interval.clone()),
                                )),
                            };
                            if tx.send(Ok(msg)).is_err() {
                                info!(
                                    "No subscribers for {output_symbol}; ending aggregation task."
                                );
                                info!("VIB aggregation task ended for {}", output_symbol);
                                VIBS_ACTIVE_AGGREGATIONS.dec();
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

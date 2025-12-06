use crate::config::TibsConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::market_data_message;
use crate::proto::{Candle, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamManager, StreamWorker};
use crate::telemetry::{TIBS_ACTIVE_AGGREGATIONS, TIBS_GENERATED};
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::{Request, Status};
use tracing::{error, info, warn};

/// Direction of tick imbalance
#[derive(Clone, Copy, Debug)]
pub enum TickDirection {
    Buy,
    Sell,
}

impl TickDirection {
    #[inline]
    pub fn to_i32(&self) -> i32 {
        match self {
            TickDirection::Buy => 1,
            TickDirection::Sell => -1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TickImbalanceBar {
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub trade_size: f64,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub tick_size: u32,
    pub theta: i32,
    pub bar_open: bool,
}

impl TickImbalanceBar {
    pub fn new(
        symbol: String,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Self {
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
            theta: b_t.to_i32(),
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
            self.theta += b_t.to_i32();
        }
    }

    fn close(&mut self) {
        self.bar_open = false;
    }

    fn p_buy(&self) -> f64 {
        // This is a simplified approximation of p_buy based on current state
        // In a real implementation, we'd track buy_ticks explicitly if needed for logic
        // For now, assuming theta roughly correlates to buy_ticks - sell_ticks
        // buy_ticks + sell_ticks = tick_size
        // buy_ticks - sell_ticks = theta
        // 2 * buy_ticks = tick_size + theta
        // buy_ticks = (tick_size + theta) / 2
        let buy_ticks = (self.tick_size as i32 + self.theta) as f64 / 2.0;
        buy_ticks / self.tick_size as f64
    }

    fn to_proto(&self) -> Candle {
        Candle {
            symbol: self.symbol.clone(),
            timestamp: self.open_time.timestamp_millis(),
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.trade_size,
            interval: "tib".to_string(),
        }
    }
}

pub struct TickImbalanceState {
    pub threshold: f64,
    pub size_ewma: f64,
    pub imbl_ewma: f64,
    pub alpha_size: f64,
    pub alpha_imbl: f64,
    pub size_boundary: (f64, f64),
    pub current_bar: Option<TickImbalanceBar>,
}

impl TickImbalanceState {
    pub fn new(
        initial_size: f64,
        initial_p_buy: f64,
        alpha_size: f64,
        alpha_imbl: f64,
        size_boundary: (f64, f64),
    ) -> Self {
        let size_ewma = initial_size;
        let imbl_ewma = 2.0 * initial_p_buy - 1.0;
        let threshold = size_ewma * imbl_ewma;

        Self {
            threshold,
            size_ewma,
            imbl_ewma,
            alpha_size,
            alpha_imbl,
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
    ) -> Option<TickImbalanceBar> {
        if self.current_bar.is_none() {
            self.current_bar = Some(TickImbalanceBar::new(symbol, price, volume, b_t, ts));
            return None;
        }

        let bar = self.current_bar.as_mut().unwrap();
        bar.update(price, volume, b_t, ts);

        if (bar.theta as f64).abs() > self.threshold.abs() {
            bar.close();
            let closed = self.current_bar.take().unwrap();

            let new_imbl = 2.0 * closed.p_buy() - 1.0;
            self.imbl_ewma = self.alpha_imbl * new_imbl + (1.0 - self.alpha_imbl) * self.imbl_ewma;

            let new_size = closed.tick_size as f64;
            self.size_ewma = self.alpha_size * new_size + (1.0 - self.alpha_size) * self.size_ewma;
            self.size_ewma =
                Self::bound(self.size_ewma, self.size_boundary.0, self.size_boundary.1);

            self.threshold = self.size_ewma * self.imbl_ewma;

            return Some(closed);
        }

        None
    }
}

#[derive(Clone)]
pub struct TibsWorker {
    upstreams: HashMap<String, String>,
    config: TibsConfig,
}

#[tonic::async_trait]
impl StreamWorker for TibsWorker {
    async fn run(
        &self,
        symbol_key: String,
        tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
    ) {
        let parts: Vec<&str> = symbol_key.split(':').collect();
        let (symbol, exchange) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            (parts[0], "BINANCE_SPOT")
        };

        // Try to find the upstream URL for the requested exchange, or fall back to BINANCE_SPOT
        let upstream_url = if let Some(url) = self.upstreams.get(exchange) {
            url.clone()
        } else {
            warn!("No upstream configured for exchange '{}'. Defaulting to BINANCE_SPOT if available.", exchange);
            if let Some(url) = self.upstreams.get("BINANCE_SPOT") {
                url.clone()
            } else {
                error!("No upstream configuration found for BINANCE_SPOT. Cannot start aggregation for {}", symbol_key);
                return;
            }
        };

        run_tib_aggregation(
            upstream_url,
            self.config.clone(),
            symbol.to_string(),
            symbol_key,
            tx,
        )
        .await;
    }
}

pub type TibsService = StreamManager<TibsWorker>;

pub fn new(upstreams: HashMap<String, String>, config: TibsConfig) -> TibsService {
    let worker = TibsWorker { upstreams, config };
    StreamManager::new(Arc::new(worker), 100, true)
}

async fn run_tib_aggregation(
    upstream_url: String,
    config: TibsConfig,
    subscription_symbol: String,
    output_symbol: String,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
) {
    info!(
        "Starting TIB aggregation for {} using upstream {}",
        output_symbol, upstream_url
    );
    TIBS_ACTIVE_AGGREGATIONS.inc();

    let mut client = match MarketDataClient::connect(upstream_url).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to upstream: {}", e);
            TIBS_ACTIVE_AGGREGATIONS.dec();
            return;
        }
    };

    let request = Request::new(MarketDataRequest {
        symbol: subscription_symbol.clone(),
        data_type: DataType::Trade as i32,
    });

    let mut stream = match client.subscribe(request).await {
        Ok(res) => res.into_inner(),
        Err(e) => {
            error!("Failed to subscribe to trades: {}", e);
            TIBS_ACTIVE_AGGREGATIONS.dec();
            return;
        }
    };

    // Initialize State with config
    let mut state = TickImbalanceState::new(
        config.initial_size,
        config.initial_p_buy,
        config.alpha_size,
        config.alpha_imbl,
        (config.size_min, config.size_max),
    );

    while let Ok(Some(msg)) = stream.message().await {
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
                TIBS_GENERATED.with_label_values(&[&output_symbol]).inc();
                let msg = MarketDataMessage {
                    exchange: "raven_tibs".to_string(),
                    data: Some(market_data_message::Data::Candle(closed_bar.to_proto())),
                };
                if tx.send(Ok(msg)).is_err() {
                    break;
                }
            }
        }
    }
    info!("TIB aggregation task ended for {}", output_symbol);
    TIBS_ACTIVE_AGGREGATIONS.dec();
}

use crate::proto::market_data_client::MarketDataClient;
use crate::proto::market_data_message;
use crate::proto::{Candle, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamKey, StreamManager, StreamWorker};
use crate::telemetry::{VPIN_ACTIVE_AGGREGATIONS, VPIN_GENERATED};
use crate::utils::retry::retry_forever;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::{Request, Status};
use tracing::{error, info, warn};

/// VPIN configuration (mirrors the Python VPINConfig).
#[derive(Clone, Debug)]
pub struct VpinConfig {
    /// Volume per bucket (each bucket contains ~this much traded volume).
    pub v: f64,
    /// Rolling number of buckets for VPIN.
    pub n: usize,
    /// Rolling window (in buckets) for sigma estimation used in BVC.
    pub sigma_window: usize,
    /// Numerical floor to avoid division by zero in sigma.
    pub sigma_floor: f64,
}

impl VpinConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.v.is_nan() || self.v <= 0.0 {
            return Err("v must be > 0");
        }
        if self.n < 2 {
            return Err("n must be >= 2");
        }
        if self.sigma_window < 2 {
            return Err("sigma_window must be >= 2");
        }
        if self.sigma_floor.is_nan() || self.sigma_floor <= 0.0 {
            return Err("sigma_floor must be > 0");
        }
        Ok(())
    }
}

/// Internal equal-volume bucket (volume bar) state.
#[derive(Clone, Debug)]
struct VolumeBucket {
    start_ts: i64,
    end_ts: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume_sum: f64,
    ticks: u64,
}

impl VolumeBucket {
    fn new(ts: i64, price: f64, qty: f64) -> Self {
        Self {
            start_ts: ts,
            end_ts: ts,
            open: price,
            high: price,
            low: price,
            close: price,
            volume_sum: qty,
            ticks: 1,
        }
    }

    fn update(&mut self, ts: i64, price: f64, qty: f64) {
        self.end_ts = ts;
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume_sum += qty;
        self.ticks += 1;
    }
}

#[derive(Clone, Debug)]
pub struct ClosedBucket {
    pub start_ts: i64,
    pub end_ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// Conceptual bucket volume for VPIN computation (clipped to cfg.v).
    pub v: f64,
    /// Number of trade prints in the bucket (not used in VPIN math; useful for debugging).
    pub ticks: u64,
}

/// Streaming VPIN calculator:
/// - trades -> volume buckets -> BVC -> rolling VPIN
#[derive(Clone, Debug)]
pub struct VpinCalculator {
    cfg: VpinConfig,
    current_bucket: Option<VolumeBucket>,
    prev_close: Option<f64>,
    dp_window: VecDeque<f64>,
    oi_window: VecDeque<f64>,
    oi_sum: f64,
}

#[derive(Clone, Debug)]
pub struct VpinPoint {
    pub bucket: ClosedBucket,
    pub dp: f64,
    pub sigma: f64,
    pub v_b: f64,
    pub v_s: f64,
    pub oi: f64,
    pub vpin: f64,
}

impl VpinCalculator {
    pub fn new(cfg: VpinConfig) -> Result<Self, &'static str> {
        cfg.validate()?;
        Ok(Self {
            cfg,
            current_bucket: None,
            prev_close: None,
            dp_window: VecDeque::new(),
            oi_window: VecDeque::new(),
            oi_sum: 0.0,
        })
    }

    /// Process a trade. Returns a VPIN point when a full bucket closes AND VPIN is computable.
    pub fn on_trade(&mut self, ts: i64, price: f64, qty: f64) -> Option<VpinPoint> {
        if qty <= 0.0 || !qty.is_finite() || !price.is_finite() {
            return None;
        }

        let v = self.cfg.v;

        match &mut self.current_bucket {
            None => {
                self.current_bucket = Some(VolumeBucket::new(ts, price, qty));
            }
            Some(b) => {
                b.update(ts, price, qty);
            }
        }

        let should_close = self
            .current_bucket
            .as_ref()
            .is_some_and(|b| b.volume_sum >= v);

        if !should_close {
            return None;
        }

        let closed = self.current_bucket.take().unwrap();
        let closed_bucket = ClosedBucket {
            start_ts: closed.start_ts,
            end_ts: closed.end_ts,
            open: closed.open,
            high: closed.high,
            low: closed.low,
            close: closed.close,
            // Like the Python version, treat each formed bucket as exactly V for VPIN math.
            v,
            ticks: closed.ticks,
        };

        let prev_close = self.prev_close?;
        let dp = closed_bucket.close - prev_close;
        self.prev_close = Some(closed_bucket.close);

        self.dp_window.push_back(dp);
        while self.dp_window.len() > self.cfg.sigma_window {
            self.dp_window.pop_front();
        }
        if self.dp_window.len() < self.cfg.sigma_window {
            return None;
        }

        let sigma = std_pop(&self.dp_window).max(self.cfg.sigma_floor);
        let x = dp / sigma;
        let phi = std_norm_cdf(x);

        let v_b = v * phi;
        let v_s = v - v_b;
        let oi = (v_b - v_s).abs(); // == abs(2*Vb - V)

        self.oi_window.push_back(oi);
        self.oi_sum += oi;
        while self.oi_window.len() > self.cfg.n {
            if let Some(old) = self.oi_window.pop_front() {
                self.oi_sum -= old;
            }
        }
        if self.oi_window.len() < self.cfg.n {
            return None;
        }

        let vpin = self.oi_sum / (self.cfg.n as f64 * v);
        Some(VpinPoint {
            bucket: closed_bucket,
            dp,
            sigma,
            v_b,
            v_s,
            oi,
            vpin,
        })
    }

    /// Call this once before processing trades to seed the first `prev_close`.
    /// Without a previous close, VPIN can't compute the first `dP`.
    pub fn seed_prev_close(&mut self, price: f64) {
        if price.is_finite() {
            self.prev_close = Some(price);
        }
    }

    pub fn has_prev_close(&self) -> bool {
        self.prev_close.is_some()
    }
}

fn std_pop(values: &VecDeque<f64>) -> f64 {
    let n = values.len();
    if n == 0 {
        return 0.0;
    }
    let n_f = n as f64;
    let mean = values.iter().sum::<f64>() / n_f;
    let var = values
        .iter()
        .map(|x| {
            let d = x - mean;
            d * d
        })
        .sum::<f64>()
        / n_f;
    var.sqrt()
}

#[inline]
fn std_norm_cdf(x: f64) -> f64 {
    // Φ(x) = 0.5 * (1 + erf(x / sqrt(2)))
    0.5 * (1.0 + erf(x * 0.7071067811865475))
}

/// erf approximation (Abramowitz & Stegun, 7.1.26).
#[inline]
fn erf(x: f64) -> f64 {
    // constants
    const A1: f64 = 0.254829592;
    const A2: f64 = -0.284496736;
    const A3: f64 = 1.421413741;
    const A4: f64 = -1.453152027;
    const A5: f64 = 1.061405429;
    const P: f64 = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + P * x);
    let y = 1.0
        - (((((A5 * t + A4) * t + A3) * t + A2) * t + A1) * t) * (-x * x).exp();
    sign * y
}

#[derive(Clone)]
pub struct VpinWorker {
    upstreams: HashMap<String, String>,
    cfg: VpinConfig,
    interval: String,
}

#[tonic::async_trait]
impl StreamWorker for VpinWorker {
    async fn run(&self, key: StreamKey, tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        let symbol = key.symbol.clone();
        let venue = key
            .venue
            .clone()
            .unwrap_or_else(|| "BINANCE_SPOT".to_string());
        let interval = self.interval.clone();

        let upstream_url = if let Some(url) = self.upstreams.get(&venue) {
            url.clone()
        } else {
            warn!(
                "No upstream configured for venue '{}'. Defaulting to BINANCE_SPOT if available.",
                venue
            );
            if let Some(url) = self.upstreams.get("BINANCE_SPOT") {
                url.clone()
            } else {
                error!(
                    "No upstream configuration found for BINANCE_SPOT. Cannot start VPIN for {}",
                    key
                );
                return;
            }
        };

        run_vpin_aggregation(
            upstream_url,
            self.cfg.clone(),
            symbol,
            key.to_string(),
            venue,
            tx,
            interval,
        )
        .await;
    }
}

pub type VpinService = StreamManager<VpinWorker>;

pub fn new(upstreams: HashMap<String, String>, cfg: VpinConfig, interval: String) -> VpinService {
    let worker = VpinWorker {
        upstreams,
        cfg,
        interval,
    };
    StreamManager::new(Arc::new(worker), 100, true)
}

async fn run_vpin_aggregation(
    upstream_url: String,
    cfg: VpinConfig,
    subscription_symbol: String,
    output_symbol: String,
    venue: String,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
    interval: String,
) {
    info!(
        "Starting VPIN aggregation (V={}, n={}, sigma_window={}) for {} using upstream {}",
        cfg.v, cfg.n, cfg.sigma_window, output_symbol, upstream_url
    );
    VPIN_ACTIVE_AGGREGATIONS.inc();

    let mut calc = match VpinCalculator::new(cfg.clone()) {
        Ok(c) => c,
        Err(e) => {
            error!("Invalid VPIN config for {output_symbol}: {e}");
            VPIN_ACTIVE_AGGREGATIONS.dec();
            return;
        }
    };

    loop {
        let mut stream = retry_forever(
            &format!("vpin connect+subscribe ({output_symbol})"),
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
                        // Seed prev_close on the first seen trade price to mirror pandas diff behavior
                        // (first dP is undefined, so we avoid emitting until we have enough history).
                        if !calc.has_prev_close() {
                            calc.seed_prev_close(trade.price);
                            continue;
                        }

                        if let Some(point) =
                            calc.on_trade(trade.timestamp, trade.price, trade.quantity)
                        {
                            VPIN_GENERATED.with_label_values(&[&output_symbol]).inc();
                            let candle = Candle {
                                symbol: subscription_symbol.clone(),
                                // Persist as the bucket start (like timebars); end_ts is still available in logs.
                                timestamp: point.bucket.start_ts,
                                open: point.bucket.open,
                                high: point.bucket.high,
                                low: point.bucket.low,
                                close: point.bucket.close,
                                volume: point.bucket.v,
                                interval: interval.clone(),
                                // Not meaningful for VPIN; keep 0 for compatibility.
                                buy_ticks: 0,
                                sell_ticks: 0,
                                total_ticks: point.bucket.ticks,
                                // We carry VPIN in `theta` to avoid proto changes.
                                theta: point.vpin,
                            };

                            let out = MarketDataMessage {
                                // Legacy proto field; prefer `producer` + `venue`.
                                exchange: String::new(),
                                venue: venue.clone(),
                                producer: "raven_vpin".to_string(),
                                data: Some(market_data_message::Data::Candle(candle)),
                            };

                            if tx.send(Ok(out)).is_err() {
                                info!(
                                    "No subscribers for {output_symbol}; ending VPIN aggregation task."
                                );
                                info!("VPIN aggregation task ended for {}", output_symbol);
                                VPIN_ACTIVE_AGGREGATIONS.dec();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vpin_streaming_matches_expected_shape() {
        let cfg = VpinConfig {
            v: 10.0,
            n: 2,
            sigma_window: 2,
            sigma_floor: 1e-12,
        };
        let mut c = VpinCalculator::new(cfg).unwrap();

        // Seed with first observed trade price.
        c.seed_prev_close(100.0);

        // Bucket 1: qty 6 + 5 => closes, but no sigma history yet => no VPIN.
        assert!(c.on_trade(1, 100.0, 6.0).is_none());
        assert!(c.on_trade(2, 101.0, 5.0).is_none());

        // Bucket 2: closes, dp history reaches sigma_window, but OI window len=1 => no VPIN.
        assert!(c.on_trade(3, 102.0, 10.0).is_none());

        // Bucket 3: closes, OI window len=2 => VPIN defined.
        let p = c.on_trade(4, 100.0, 10.0).expect("expected VPIN point");
        assert!(p.vpin.is_finite());
        // Hand-computed approx for this toy stream ≈ 0.9082
        assert!((p.vpin - 0.9082).abs() < 1e-3, "got {}", p.vpin);
    }
}



use crate::config::InfluxConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::{market_data_message, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamDataType, StreamKey, StreamManager, StreamWorker};
use crate::telemetry::{INFLUX_ACTIVE_TASKS, INFLUX_POINTS_WRITTEN};
use influxdb2::models::DataPoint;
use influxdb2::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{self, Duration};
use tonic::Status;
use tracing::{error, info, warn};

struct InfluxActiveGuard;
impl Drop for InfluxActiveGuard {
    fn drop(&mut self) {
        INFLUX_ACTIVE_TASKS.dec();
    }
}

#[derive(Clone)]
pub struct InfluxWorker {
    upstreams: Arc<HashMap<String, String>>,
    default_upstream: String,
    client: Client,
    bucket: String,
    batch_size: usize,
    batch_interval_ms: u64,
}

#[tonic::async_trait]
impl StreamWorker for InfluxWorker {
    async fn run(&self, key: StreamKey, _tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        let symbol = key.symbol.clone();
        let exchange = key.venue.clone().unwrap_or_default();
        let data_type = match key.data_type {
            StreamDataType::Trade => DataType::Trade,
            StreamDataType::Orderbook => DataType::Orderbook,
            StreamDataType::Candle => DataType::Candle,
            StreamDataType::Funding => DataType::Funding,
            StreamDataType::Unknown(_) => DataType::Unknown,
        };

        // Select upstream
        let upstream_url = if !exchange.is_empty() {
            self.upstreams.get(&exchange).cloned().unwrap_or_else(|| {
                warn!(
                    "No upstream found for exchange '{}', falling back to default",
                    exchange
                );
                self.default_upstream.clone()
            })
        } else {
            self.default_upstream.clone()
        };

        run_persistence(
            upstream_url,
            symbol,
            exchange,
            key.to_string(),
            data_type,
            self.client.clone(),
            self.bucket.clone(),
            self.batch_size,
            self.batch_interval_ms,
        )
        .await;
    }
}

pub type PersistenceService = StreamManager<InfluxWorker>;

pub fn new(
    default_upstream: String,
    upstreams: HashMap<String, String>,
    config: InfluxConfig,
) -> PersistenceService {
    let client = Client::new(&config.url, &config.org, &config.token);
    let bucket = config.bucket.clone();

    let worker = InfluxWorker {
        upstreams: Arc::new(upstreams),
        default_upstream,
        client,
        bucket,
        batch_size: config.batch_size,
        batch_interval_ms: config.batch_interval_ms,
    };

    StreamManager::new(Arc::new(worker), 10000, false)
}

#[allow(clippy::too_many_arguments)]
async fn run_persistence(
    upstream_url: String,
    symbol: String,
    exchange: String,
    key: String, // For logging
    data_type: DataType,
    influx_client: Client,
    bucket: String,
    batch_size: usize,
    batch_interval_ms: u64,
) {
    let key_clone = key.clone();
    INFLUX_ACTIVE_TASKS.inc();
    let _active_guard = InfluxActiveGuard;

    info!("Persistence task started for {}", key_clone);

    // Create a channel to decouple reading from writing
    // High capacity to handle bursts
    let (tx, mut rx) = mpsc::channel::<DataPoint>(10000);

    // Spawn writer task
    let writer_client = influx_client.clone();
    let writer_bucket = bucket.clone();
    let writer_key = key_clone.clone();

    let _writer_handle = tokio::spawn(async move {
        let mut buffer: Vec<DataPoint> = Vec::with_capacity(batch_size);
        let mut interval = time::interval(Duration::from_millis(batch_interval_ms));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                Some(point) = rx.recv() => {
                    buffer.push(point);
                    if buffer.len() >= batch_size {
                        flush_buffer(&writer_client, &writer_bucket, &mut buffer).await;
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        flush_buffer(&writer_client, &writer_bucket, &mut buffer).await;
                    }
                }
                else => {
                    // Channel closed
                    break;
                }
            }
        }
        // Flush remaining
        if !buffer.is_empty() {
            flush_buffer(&writer_client, &writer_bucket, &mut buffer).await;
        }
        info!("Persistence writer task ended for {}", writer_key);
    });

    loop {
        // (Re)connect + (re)subscribe. This will initially fail until the upstream stream
        // has been started via Control, which supports "wire first, subscribe later".
        let mut client = match MarketDataClient::connect(upstream_url.clone()).await {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "Failed to connect to upstream {}: {}. Retrying in 2s...",
                    upstream_url, e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        let request = tonic::Request::new(MarketDataRequest {
            symbol: symbol.clone(),
            data_type: data_type as i32,
            venue: exchange.clone(),
        });

        let mut stream = match client.subscribe(request).await {
            Ok(res) => res.into_inner(),
            Err(e) => {
                warn!(
                    "Failed to subscribe to upstream ({}): {}. Retrying in 2s...",
                    key_clone, e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        // Reader loop - purely reads and pushes to channel
        loop {
            match stream.message().await {
                Ok(Some(msg)) => match msg.data {
                    Some(market_data_message::Data::Trade(trade)) => {
                        let timestamp_ns = trade.timestamp.saturating_mul(1_000_000);

                        let mut point_builder = DataPoint::builder("trades")
                            .tag("symbol", &trade.symbol)
                            .tag("side", &trade.side)
                            .field("price", trade.price)
                            .field("quantity", trade.quantity)
                            .timestamp(timestamp_ns);

                        if !exchange.is_empty() {
                            point_builder = point_builder.tag("exchange", &exchange);
                        }

                        if let Ok(point) = point_builder.build() {
                            if tx.try_send(point).is_err() {
                                warn!("Persistence buffer full for {}. Dropping point.", key_clone);
                            }
                        }
                    }
                    Some(market_data_message::Data::Orderbook(book)) => {
                        // Store orderbook snapshot (best bid/ask + derived values).
                        let timestamp_ns = book.timestamp.saturating_mul(1_000_000);

                        let best_bid = book.bids.first();
                        let best_ask = book.asks.first();

                        let mut point_builder = DataPoint::builder("orderbook")
                            .tag("symbol", &book.symbol)
                            .timestamp(timestamp_ns);

                        if !exchange.is_empty() {
                            point_builder = point_builder.tag("exchange", &exchange);
                        }

                        if let Some(bid) = best_bid {
                            point_builder = point_builder
                                .field("bid_price", bid.price)
                                .field("bid_qty", bid.quantity);
                        }

                        if let Some(ask) = best_ask {
                            point_builder = point_builder
                                .field("ask_price", ask.price)
                                .field("ask_qty", ask.quantity);
                        }

                        // Calculate spread and mid price if both exist
                        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                            let spread = ask.price - bid.price;
                            let mid = (ask.price + bid.price) / 2.0;
                            point_builder = point_builder
                                .field("spread", spread)
                                .field("mid_price", mid);

                            // Calculate simple imbalance (Bid Qty / (Bid Qty + Ask Qty))
                            let imbalance = bid.quantity / (bid.quantity + ask.quantity);
                            point_builder = point_builder.field("imbalance", imbalance);
                        }

                        if let Ok(point) = point_builder.build() {
                            if tx.try_send(point).is_err() {
                                warn!("Persistence buffer full for {}. Dropping point.", key_clone);
                            }
                        }
                    }
                    _ => {}
                },
                Ok(None) => {
                    warn!(
                        "Upstream stream ended for {}. Reconnecting in 2s...",
                        key_clone
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    break;
                }
                Err(e) => {
                    warn!(
                        "Error receiving message for {}: {}. Reconnecting in 2s...",
                        key_clone, e
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    break;
                }
            }
        }
    }
}

async fn flush_buffer(client: &Client, bucket: &str, buffer: &mut Vec<DataPoint>) {
    if buffer.is_empty() {
        return;
    }

    let mut retries = 0;
    const MAX_RETRIES: u32 = 3;
    let mut backoff = Duration::from_millis(100);

    loop {
        let points = buffer.clone();

        if let Err(e) = client.write(bucket, tokio_stream::iter(points)).await {
            retries += 1;
            if retries > MAX_RETRIES {
                error!(
                    "PERSISTENT FAILURE writing to InfluxDB after {} retries. Dropping {} points. Error: {}",
                    MAX_RETRIES,
                    buffer.len(),
                    e
                );
                buffer.clear();
                return;
            }
            warn!("Failed to write to InfluxDB (retry {retries}/{MAX_RETRIES}): {e}. Retrying in {backoff:?}...");
            time::sleep(backoff).await;
            backoff *= 2;
        } else {
            let count = buffer.len();
            INFLUX_POINTS_WRITTEN
                .with_label_values(&[bucket])
                .inc_by(count as u64);
            buffer.clear();
            return;
        }
    }
}

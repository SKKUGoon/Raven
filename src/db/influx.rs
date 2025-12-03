use crate::config::InfluxConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::{market_data_message, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamManager, StreamWorker};
use crate::telemetry::{INFLUX_ACTIVE_TASKS, INFLUX_POINTS_WRITTEN};
use influxdb2::models::DataPoint;
use influxdb2::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{self, Duration};
use tonic::Status;
use tracing::{error, info, warn};

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
    async fn run(&self, key: String, _tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        // Parse key (SYMBOL:EXCHANGE or just SYMBOL)
        let (symbol, exchange) = match key.split_once(':') {
            Some((s, e)) => (s.to_string(), e.to_string()),
            None => (key.clone(), String::new()),
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
            key,
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
    influx_client: Client,
    bucket: String,
    batch_size: usize,
    batch_interval_ms: u64,
) {
    let mut client = match MarketDataClient::connect(upstream_url.clone()).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to upstream {}: {}", upstream_url, e);
            return;
        }
    };

    let request = tonic::Request::new(MarketDataRequest {
        symbol: symbol.clone(),
        data_type: DataType::Trade as i32,
    });

    let mut stream = match client.subscribe(request).await {
        Ok(res) => res.into_inner(),
        Err(e) => {
            error!("Failed to subscribe to upstream: {}", e);
            return;
        }
    };

    let key_clone = key.clone();
    INFLUX_ACTIVE_TASKS.inc();

    info!("Persistence task started for {}", key_clone);

    // Create a channel to decouple reading from writing
    // High capacity to handle bursts
    let (tx, mut rx) = mpsc::channel::<DataPoint>(10000);

    // Spawn writer task
    let writer_client = influx_client.clone();
    let writer_bucket = bucket.clone();
    let writer_key = key_clone.clone();

    let writer_handle = tokio::spawn(async move {
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

    // Reader loop - purely reads and pushes to channel
    loop {
        match stream.message().await {
            Ok(Some(msg)) => {
                if let Some(market_data_message::Data::Trade(trade)) = msg.data {
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
                        // If channel is full, we drop the point to avoid blocking the stream
                        // Alternatively, we could block, but that causes "Stream lagged" upstream.
                        // Dropping is better for stability than crashing/lagging.
                        if tx.try_send(point).is_err() {
                            warn!("Persistence buffer full for {}. Dropping point.", key_clone);
                        }
                    }
                }
            }
            Ok(None) => {
                info!("Stream ended for {}", key_clone);
                break;
            }
            Err(e) => {
                error!("Error receiving message for {}: {}", key_clone, e);
                break;
            }
        }
    }

    // Drop tx to signal writer to finish
    drop(tx);
    // Wait for writer to finish flushing
    let _ = writer_handle.await;

    info!("Persistence task ended for {}", key_clone);
    INFLUX_ACTIVE_TASKS.dec();
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
            warn!(
                "Failed to write to InfluxDB (retry {}/{}): {}. Retrying in {:?}...",
                retries, MAX_RETRIES, e, backoff
            );
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

use crate::config::InfluxConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::{market_data_message, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamManager, StreamWorker};
use crate::telemetry::{INFLUX_ACTIVE_TASKS, INFLUX_POINTS_WRITTEN};
use influxdb2::models::DataPoint;
use influxdb2::Client;
use tokio::sync::broadcast;
use tokio::time::{self, Duration};
use tonic::Status;
use tracing::{error, info, warn};
use std::sync::Arc;

#[derive(Clone)]
pub struct InfluxWorker {
    upstream_url: String,
    client: Client,
    bucket: String,
    batch_size: usize,
    batch_interval_ms: u64,
}

#[tonic::async_trait]
impl StreamWorker for InfluxWorker {
    async fn run(&self, symbol: String, _tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        run_persistence(
            self.upstream_url.clone(),
            symbol,
            self.client.clone(),
            self.bucket.clone(),
            self.batch_size,
            self.batch_interval_ms,
        )
        .await;
    }
}

pub type PersistenceService = StreamManager<InfluxWorker>;

pub fn new(upstream_url: String, config: InfluxConfig) -> PersistenceService {
    let client = Client::new(&config.url, &config.org, &config.token);
    let bucket = config.bucket.clone();

    let worker = InfluxWorker {
        upstream_url,
        client,
        bucket,
        batch_size: config.batch_size,
        batch_interval_ms: config.batch_interval_ms,
    };

    StreamManager::new(Arc::new(worker), 100, false)
}

async fn run_persistence(
    upstream_url: String,
    symbol: String,
    influx_client: Client,
    bucket: String,
    batch_size: usize,
    batch_interval_ms: u64,
) {
    let mut client = match MarketDataClient::connect(upstream_url).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to upstream: {}", e);
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

    let symbol_clone = symbol.clone();
    INFLUX_ACTIVE_TASKS.inc();

    info!("Persistence task started for {}", symbol_clone);

    let mut buffer: Vec<DataPoint> = Vec::with_capacity(batch_size);
    let mut interval = time::interval(Duration::from_millis(batch_interval_ms));
    // Set missed tick behavior to skip to avoid bursts after lag
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            res = stream.message() => {
                match res {
                    Ok(Some(msg)) => {
                         if let Some(market_data_message::Data::Trade(trade)) = msg.data {
                            let point_builder = DataPoint::builder("trades")
                                .tag("symbol", &trade.symbol)
                                .tag("side", &trade.side)
                                .field("price", trade.price)
                                .field("quantity", trade.quantity)
                                .timestamp(trade.timestamp);

                             if let Ok(point) = point_builder.build() {
                                buffer.push(point);
                                if buffer.len() >= batch_size {
                                    flush_buffer(&influx_client, &bucket, &mut buffer).await;
                                }
                             }
                         }
                    }
                    Ok(None) => {
                        // Stream ended
                        break;
                    }
                    Err(e) => {
                        error!("Error receiving message: {}", e);
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                if !buffer.is_empty() {
                    flush_buffer(&influx_client, &bucket, &mut buffer).await;
                }
            }
        }
    }

    // Flush remaining points on exit
    if !buffer.is_empty() {
        flush_buffer(&influx_client, &bucket, &mut buffer).await;
    }

    info!("Persistence task ended for {}", symbol_clone);
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
        // Clone points for the attempt so we keep the original buffer in case of failure
        // Note: DataPoint must be Clone.
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
                // Drop data (Dead Letter Queue would go here)
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
            INFLUX_POINTS_WRITTEN.with_label_values(&[bucket]).inc_by(count as u64);
            buffer.clear();
            return;
        }
    }
}

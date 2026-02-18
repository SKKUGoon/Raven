use crate::config::InfluxConfig;
use crate::proto::MarketDataMessage;
use crate::service::{StreamKey, StreamManager, StreamWorker};
use influxdb2::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::Status;

use super::persistence::run_persistence;

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
        let data_type = key.data_type.to_proto();

        let upstream_url = if !exchange.is_empty() {
            let composite = format!("{}{}", exchange, key.data_type.suffix());
            self.upstreams
                .get(&composite)
                .or_else(|| self.upstreams.get(&exchange))
                .cloned()
                .unwrap_or_else(|| {
                    tracing::warn!(
                        "No upstream found for '{}' (tried '{}'), falling back to default",
                        exchange,
                        composite,
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

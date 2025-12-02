use crate::config::InfluxConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::{market_data_message, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::StreamManager;
use influxdb2::models::DataPoint;
use influxdb2::Client;
use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, register_int_gauge, IntCounterVec, IntGauge};
use std::future::Future;
use std::pin::Pin;
use tokio::sync::broadcast;
use tonic::Status;
use tracing::{error, info};

lazy_static! {
    static ref POINTS_WRITTEN: IntCounterVec = register_int_counter_vec!(
        "raven_persistence_points_written_total",
        "Total number of data points written to InfluxDB",
        &["bucket"]
    )
    .unwrap();
    static ref ACTIVE_TASKS: IntGauge = register_int_gauge!(
        "raven_persistence_active_tasks",
        "Number of active persistence tasks"
    )
    .unwrap();
}

pub type PersistenceService = StreamManager<
    Box<
        dyn Fn(
                String,
                broadcast::Sender<Result<MarketDataMessage, Status>>,
            ) -> Pin<Box<dyn Future<Output = ()> + Send>>
            + Send
            + Sync,
    >,
>;

pub fn new(upstream_url: String, config: InfluxConfig) -> PersistenceService {
    let client = Client::new(&config.url, &config.org, &config.token);
    let bucket = config.bucket.clone();

    StreamManager::new(Box::new(move |symbol, _tx| {
        let client = client.clone();
        let bucket = bucket.clone();
        let upstream_url = upstream_url.clone();

        Box::pin(async move {
            run_persistence(upstream_url, symbol, client, bucket).await;
        })
    }))
}

async fn run_persistence(
    upstream_url: String,
    symbol: String,
    influx_client: Client,
    bucket: String,
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
    ACTIVE_TASKS.inc();

    info!("Persistence task started for {}", symbol_clone);
    while let Ok(Some(msg)) = stream.message().await {
        if let Some(market_data_message::Data::Trade(trade)) = msg.data {
            let point = DataPoint::builder("trades")
                .tag("symbol", &trade.symbol)
                .tag("side", &trade.side)
                .field("price", trade.price)
                .field("quantity", trade.quantity)
                .timestamp(trade.timestamp)
                .build();

            if let Ok(point) = point {
                if let Err(e) = influx_client
                    .write(&bucket, tokio_stream::iter(vec![point]))
                    .await
                {
                    error!("Failed to write to InfluxDB: {}", e);
                } else {
                    POINTS_WRITTEN.with_label_values(&[&bucket]).inc();
                }
            }
        }
    }
    info!("Persistence task ended for {}", symbol_clone);
    ACTIVE_TASKS.dec();
}

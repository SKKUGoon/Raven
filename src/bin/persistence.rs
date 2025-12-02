use dashmap::DashMap;
use influxdb2::models::DataPoint;
use influxdb2::Client;
use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, register_int_gauge, IntCounterVec, IntGauge};
use raven::proto::control_server::Control;
use raven::proto::market_data_client::MarketDataClient;
use raven::proto::{
    market_data_message, ControlRequest, ControlResponse, DataType, ListRequest, ListResponse,
    MarketDataRequest, StopAllRequest, StopAllResponse,
};
use raven::service::RavenService;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tonic::{Request, Response, Status};
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

#[derive(Clone)]
struct PersistenceService {
    tasks: Arc<DashMap<String, JoinHandle<()>>>,
    upstream_url: String,
    influx_client: Client,
    bucket: String,
    // org: String, // Currently unused, but part of influx client setup internally
}

impl PersistenceService {
    fn new(upstream_url: String) -> Self {
        // Hardcoded for now, should come from config
        let influx_url =
            std::env::var("INFLUX_URL").unwrap_or_else(|_| "http://localhost:8086".to_string());
        let influx_token = std::env::var("INFLUX_TOKEN").unwrap_or_else(|_| "my-token".to_string());
        let org = std::env::var("INFLUX_ORG").unwrap_or_else(|_| "my-org".to_string());
        let bucket = std::env::var("INFLUX_BUCKET").unwrap_or_else(|_| "raven".to_string());

        let client = Client::new(influx_url, org, influx_token); // org moved here

        Self {
            tasks: Arc::new(DashMap::new()),
            upstream_url,
            influx_client: client,
            bucket,
            // org,
        }
    }

    async fn start_persisting(&self, symbol: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = MarketDataClient::connect(self.upstream_url.clone()).await?;
        let request = Request::new(MarketDataRequest {
            symbol: symbol.clone(),
            data_type: DataType::Trade as i32,
        });

        let mut stream = client.subscribe(request).await?.into_inner();
        let symbol_clone = symbol.clone();
        let influx_client = self.influx_client.clone();
        let bucket = self.bucket.clone();

        ACTIVE_TASKS.inc();

        let handle = tokio::spawn(async move {
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
        });

        self.tasks.insert(symbol, handle);
        Ok(())
    }
}

#[tonic::async_trait]
impl Control for PersistenceService {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        info!("Control: Start persistence for {symbol}");

        if self.tasks.contains_key(&symbol) {
            return Ok(Response::new(ControlResponse {
                success: true,
                message: format!("Already persisting {symbol}"),
            }));
        }

        // Clone self to move into async block if needed, but here we just call method
        // But start_persisting needs &self, async.
        // We can't easily spawn logic that borrows self inside start_collection without cloning.
        // Actually `start_persisting` spawns a task, but `MarketDataClient::connect` is async.
        // We can wait for connection here.

        match self.start_persisting(symbol.clone()).await {
            Ok(_) => Ok(Response::new(ControlResponse {
                success: true,
                message: format!("Started persistence for {symbol}"),
            })),
            Err(e) => {
                error!("Failed to start persistence: {e}");
                Ok(Response::new(ControlResponse {
                    success: false,
                    message: format!("Failed to start persistence: {e}"),
                }))
            }
        }
    }

    async fn stop_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        if let Some((_, handle)) = self.tasks.remove(&symbol) {
            handle.abort(); // Simple cancellation
            info!("Control: Stopped persistence for {symbol}");
            Ok(Response::new(ControlResponse {
                success: true,
                message: format!("Stopped persistence for {symbol}"),
            }))
        } else {
            Ok(Response::new(ControlResponse {
                success: false,
                message: format!("No persistence task found for {symbol}"),
            }))
        }
    }

    async fn list_collections(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        use raven::proto::CollectionInfo;
        let collections = self
            .tasks
            .iter()
            .map(|entry| CollectionInfo {
                symbol: entry.key().clone(),
                status: "active".to_string(),
                subscriber_count: 1,
            })
            .collect();

        Ok(Response::new(ListResponse { collections }))
    }

    async fn stop_all_collections(
        &self,
        _request: Request<StopAllRequest>,
    ) -> Result<Response<StopAllResponse>, Status> {
        for entry in self.tasks.iter() {
            entry.value().abort();
        }
        self.tasks.clear();
        info!("Control: Stopped all persistence");
        Ok(Response::new(StopAllResponse {
            success: true,
            message: "All persistence stopped".to_string(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Persistence runs on 50052 by default, upstream on 50051
    let addr = "0.0.0.0:50052".parse()?;
    let upstream =
        std::env::var("UPSTREAM_URL").unwrap_or_else(|_| "http://localhost:50051".to_string());

    let service_impl = PersistenceService::new(upstream);
    let raven = RavenService::new("Persistence", service_impl.clone());

    raven.serve(addr).await?;

    Ok(())
}

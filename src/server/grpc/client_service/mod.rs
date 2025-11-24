pub mod grpc_service;
pub mod manager;

pub use grpc_service::MarketDataServiceImpl;
pub use manager::ClientManager;

use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;

use crate::common::db::influx_client::InfluxClient;
use crate::common::error::RavenResult;
use crate::proto::market_data_service_server::MarketDataServiceServer;
use crate::server::data_engine::storage::HighFrequencyStorage;
use crate::server::prometheus::MetricsCollector;
use crate::server::stream_router::StreamRouter;

/// Market Data Server - Main gRPC server implementation
#[derive(Clone)]
pub struct MarketDataServer {
    /// Stream Router - manages all client subscriptions and data routing
    stream_router: Arc<StreamRouter>,
    /// InfluxDB client for historical data
    influx_client: Arc<InfluxClient>,
    /// High-frequency atomic storage for real-time data
    hf_storage: Arc<HighFrequencyStorage>,
    /// Metrics collector for monitoring
    metrics: Option<Arc<MetricsCollector>>,
    /// Client manager
    client_manager: Arc<ClientManager>,
}

impl MarketDataServer {
    /// Create a new MarketDataServer instance
    pub fn new(
        stream_router: Arc<StreamRouter>,
        influx_client: Arc<InfluxClient>,
        hf_storage: Arc<HighFrequencyStorage>,
        client_manager: Arc<ClientManager>,
    ) -> Self {
        info!("▲ Market Data Server is assembling...");

        let max_connections = client_manager.get_max_clients();
        info!("⚔ Maximum concurrent connections: {}", max_connections);

        Self {
            stream_router,
            influx_client,
            hf_storage,
            metrics: None,
            client_manager,
        }
    }

    /// Create a new MarketDataServer instance with metrics
    pub fn with_metrics(
        stream_router: Arc<StreamRouter>,
        influx_client: Arc<InfluxClient>,
        hf_storage: Arc<HighFrequencyStorage>,
        metrics: Arc<MetricsCollector>,
        client_manager: Arc<ClientManager>,
    ) -> Self {
        info!("▲ Market Data Server is assembling with monitoring...");

        let max_connections = client_manager.get_max_clients();
        info!("⚔ Maximum concurrent connections: {}", max_connections);

        Self {
            stream_router,
            influx_client,
            hf_storage,
            metrics: Some(metrics),
            client_manager,
        }
    }

    /// Start the gRPC server
    pub async fn start(self, host: &str, port: u16) -> RavenResult<()> {
        let addr = format!("{host}:{port}").parse().map_err(|e| {
            crate::raven_error!(
                configuration,
                format!("Invalid gRPC bind address {host}:{port}: {e}")
            )
        })?;

        info!("▲ Market Data Server is taking position at {}", addr);
        info!("◦ Data streams are ready to carry messages");

        let service_impl = MarketDataServiceImpl::new(
            self.stream_router,
            self.influx_client,
            self.hf_storage,
            self.metrics,
            self.client_manager,
        );

        let service = MarketDataServiceServer::new(service_impl)
            .max_decoding_message_size(4 * 1024 * 1024) // 4MB max message size
            .max_encoding_message_size(4 * 1024 * 1024);

        info!("▶ Starting gRPC server...");

        Server::builder()
            .add_service(service)
            .serve(addr)
            .await
            .map_err(|e| crate::raven_error!(grpc_connection, format!("Server failed: {e}")))?;

        info!("■ gRPC server stopped");
        Ok(())
    }
}

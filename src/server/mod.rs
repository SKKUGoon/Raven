// The Night's Watch - gRPC Server Implementation
// "The watchers on the wall who guard the realm of market data"

use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;

// Import our protobuf definitions and other modules
use crate::database::influx_client::InfluxClient;
use crate::monitoring::MetricsCollector;
use crate::proto::market_data_service_server::MarketDataServiceServer;
use crate::subscription_manager::SubscriptionManager;
use crate::types::HighFrequencyStorage;

pub mod connection;
pub mod grpc_service;

#[cfg(test)]
pub mod tests;

pub use connection::ConnectionManager;
pub use grpc_service::MarketDataServiceImpl;

use crate::error::{RavenError, RavenResult};

/// The Night's Watch - Main gRPC server implementation
pub struct MarketDataServer {
    /// The Maester's Registry - manages all client subscriptions
    subscription_manager: Arc<SubscriptionManager>,
    /// The Iron Bank - InfluxDB client for historical data
    influx_client: Arc<InfluxClient>,
    /// High-frequency atomic storage for real-time data
    hf_storage: Arc<HighFrequencyStorage>,
    /// Metrics collector for monitoring
    metrics: Option<Arc<MetricsCollector>>,
    /// Connection manager
    connection_manager: ConnectionManager,
}

impl MarketDataServer {
    /// Create a new MarketDataServer instance
    pub fn new(
        subscription_manager: Arc<SubscriptionManager>,
        influx_client: Arc<InfluxClient>,
        hf_storage: Arc<HighFrequencyStorage>,
        max_connections: usize,
    ) -> Self {
        info!("▲ The Night's Watch is assembling...");
        info!("⚔ Maximum concurrent connections: {}", max_connections);

        Self {
            subscription_manager,
            influx_client,
            hf_storage,
            metrics: None,
            connection_manager: ConnectionManager::new(max_connections),
        }
    }

    /// Create a new MarketDataServer instance with metrics
    pub fn with_metrics(
        subscription_manager: Arc<SubscriptionManager>,
        influx_client: Arc<InfluxClient>,
        hf_storage: Arc<HighFrequencyStorage>,
        metrics: Arc<MetricsCollector>,
        max_connections: usize,
    ) -> Self {
        info!("▲ The Night's Watch is assembling with monitoring...");
        info!("⚔ Maximum concurrent connections: {}", max_connections);

        Self {
            subscription_manager,
            influx_client,
            hf_storage,
            metrics: Some(metrics),
            connection_manager: ConnectionManager::new(max_connections),
        }
    }

    /// Start the gRPC server
    pub async fn start(self, host: &str, port: u16) -> RavenResult<()> {
        let addr = format!("{host}:{port}").parse().map_err(|e| {
            RavenError::configuration(format!("Invalid gRPC bind address {host}:{port}: {e}"))
        })?;

        info!("▲ The Night's Watch is taking position at {}", addr);
        info!("◦ Ravens are ready to carry messages across the realm");

        let service_impl = MarketDataServiceImpl::new(
            self.subscription_manager,
            self.influx_client,
            self.hf_storage,
            self.metrics,
            self.connection_manager,
        );

        let service = MarketDataServiceServer::new(service_impl)
            .max_decoding_message_size(4 * 1024 * 1024) // 4MB max message size
            .max_encoding_message_size(4 * 1024 * 1024);

        info!("▶ Starting gRPC server...");

        Server::builder()
            .add_service(service)
            .serve(addr)
            .await
            .map_err(|e| RavenError::grpc_connection(format!("Server failed: {e}")))?;

        info!("■ gRPC server stopped");
        Ok(())
    }
}

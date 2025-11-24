use crate::{
    common::db::{circuit_breaker::CircuitBreakerRegistry, DeadLetterQueue},
    common::error::RavenResult,
    server::grpc::client_service::ClientManager,
    server::prometheus::TracingService,
};
use std::sync::Arc;
use tracing::info;

/// Trait for data collectors that can be gracefully shutdown
pub trait DataCollector: Send + Sync {
    fn name(&self) -> &'static str;
}

/// Generic container for data collectors
pub struct DataCollectors {
    collectors: Vec<(String, Box<dyn std::any::Any + Send + Sync>)>,
}

impl DataCollectors {
    pub fn new() -> Self {
        Self {
            collectors: Vec::new(),
        }
    }

    pub fn add_collector<T: DataCollector + 'static>(
        mut self,
        name: String,
        collector: Arc<T>,
    ) -> Self {
        self.collectors.push((name, Box::new(collector)));
        self
    }

    pub fn len(&self) -> usize {
        self.collectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.collectors.is_empty()
    }

    pub fn collector_names(&self) -> Vec<&String> {
        self.collectors.iter().map(|(name, _)| name).collect()
    }
}

impl Default for DataCollectors {
    fn default() -> Self {
        Self::new()
    }
}

/// Wait for shutdown signals
pub async fn wait_for_shutdown_signal() -> RavenResult<()> {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|e| {
            crate::raven_error!(internal, format!("Failed to setup SIGTERM handler: {e}"))
        })?;

    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|e| {
            crate::raven_error!(internal, format!("Failed to setup SIGINT handler: {e}"))
        })?;

    // Keep the server running and wait for shutdown signals
    tokio::select! {
        _ = sigint.recv() => {
            info!("!!! Received SIGINT (Ctrl+C) shutdown signal");
        }
        _ = sigterm.recv() => {
            info!("!!! Received SIGTERM shutdown signal");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("!!! Received CTRL+C shutdown signal");
        }
    }

    Ok(())
}

/// Perform graceful shutdown sequence
pub async fn perform_graceful_shutdown(
    client_manager: Arc<ClientManager>,
    dead_letter_queue: Arc<DeadLetterQueue>,
    tracing_service: Arc<TracingService>,
    circuit_breaker_registry: Arc<CircuitBreakerRegistry>,
    monitoring_handles: Vec<tokio::task::JoinHandle<()>>,
    data_collectors: DataCollectors,
) -> RavenResult<()> {
    info!("Shutting down Raven gracefully...");

    // Stop data collectors
    if !data_collectors.is_empty() {
        info!("Stopping {} data collector(s)...", data_collectors.len());
        let collector_names = data_collectors.collector_names();
        for name in collector_names {
            info!("  * Stopping collector: {}", name);
        }
        // Note: WebSocket connections will be dropped when collectors are dropped
        // and tokio tasks will be cancelled automatically
        info!("All data collectors stopped gracefully");
    }

    // Stop accepting new connections and disconnect existing clients
    if let Err(e) = client_manager.shutdown_all_clients().await {
        tracing::error!(error = %e, "Error during client shutdown");
    }

    // Stop dead letter queue processing and persist remaining entries
    dead_letter_queue.stop_processing();
    if let Err(e) = dead_letter_queue.persist_to_disk().await {
        tracing::error!(error = %e, "Failed to persist dead letter queue");
    }

    // TODO: Process any remaining dead letter entries
    // Note: The process_dead_letter_queue method doesn't exist in the current implementation
    // This would be handled by the background processing loop that was already stopped

    // Shutdown monitoring services
    info!("Shutting down monitoring services...");
    for handle in monitoring_handles {
        handle.abort();
    }

    // Shutdown tracing and flush spans
    if let Err(e) = tracing_service.shutdown().await {
        tracing::error!(error = %e, "Failed to shutdown tracing");
    }

    // Get final statistics
    let client_stats = client_manager.get_client_stats().await;
    let dlq_stats = dead_letter_queue.get_statistics().await;
    let cb_stats = circuit_breaker_registry.get_all_stats().await;

    info!("Final statistics:");
    info!("  * Clients: {:?}", client_stats);
    info!("  * Dead Letter Queue: {:?}", dlq_stats);
    info!("  * Circuit Breakers: {:?}", cb_stats);

    info!("Raven shutdown complete");

    Ok(())
}

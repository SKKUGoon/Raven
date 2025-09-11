// Connection management for the gRPC server

use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::monitoring::MetricsCollector;

/// Manages active connections and enforces connection limits
#[derive(Clone)]
pub struct ConnectionManager {
    /// Maximum allowed concurrent connections
    max_connections: usize,
    /// Current active connection count
    active_connections: Arc<RwLock<usize>>,
}

impl ConnectionManager {
    /// Create a new ConnectionManager
    pub fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            active_connections: Arc::new(RwLock::new(0)),
        }
    }

    /// Check if server can accept new connections
    pub async fn can_accept_connection(&self) -> bool {
        let current_connections = *self.active_connections.read().await;
        current_connections < self.max_connections
    }

    /// Increment active connection count
    pub async fn increment_connections(
        &self,
        metrics: Option<&Arc<MetricsCollector>>,
    ) -> Result<()> {
        let mut connections = self.active_connections.write().await;
        if *connections >= self.max_connections {
            if let Some(metrics) = metrics {
                metrics.record_error("max_connections_reached", "server");
            }
            return Err(anyhow!("Maximum connections reached"));
        }
        *connections += 1;
        debug!("🔗 Active connections: {}", *connections);

        // Update metrics
        if let Some(metrics) = metrics {
            metrics.record_connection("grpc");
            metrics.active_connections.set(*connections as f64);
        }

        Ok(())
    }

    /// Decrement active connection count
    pub async fn decrement_connections(
        &self,
        duration: std::time::Duration,
        metrics: Option<&Arc<MetricsCollector>>,
    ) {
        let mut connections = self.active_connections.write().await;
        if *connections > 0 {
            *connections -= 1;
        }
        debug!("🔗 Active connections: {}", *connections);

        // Update metrics
        if let Some(metrics) = metrics {
            metrics.record_disconnection("grpc", duration);
            metrics.active_connections.set(*connections as f64);
        }
    }

    /// Get current active connection count
    pub async fn get_active_connections(&self) -> usize {
        *self.active_connections.read().await
    }

    /// Get maximum connections limit
    pub fn get_max_connections(&self) -> usize {
        self.max_connections
    }
}

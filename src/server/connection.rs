// Connection management for the gRPC server

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::debug;

use crate::error::RavenResult;
use crate::monitoring::MetricsCollector;

/// Tracks active gRPC connections and enforces the server-wide connection limit.
/// The `SubscriptionManager` owns all client subscription state; this type only
/// concerns itself with admitting or rejecting new connections.
#[derive(Clone)]
pub struct ConnectionManager {
    /// Maximum allowed concurrent connections
    max_connections: usize,
    /// Current active connection count
    active_connections: Arc<AtomicUsize>,
}

impl ConnectionManager {
    /// Create a new ConnectionManager
    pub fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            active_connections: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Check if server can accept new connections
    pub async fn can_accept_connection(&self) -> bool {
        let current_connections = self.active_connections.load(Ordering::Relaxed);
        current_connections < self.max_connections
    }

    /// Increment active connection count
    pub async fn increment_connections(
        &self,
        metrics: Option<&Arc<MetricsCollector>>,
    ) -> RavenResult<()> {
        let mut current = self.active_connections.load(Ordering::Relaxed);
        loop {
            if current >= self.max_connections {
                if let Some(metrics) = metrics {
                    metrics.record_error("max_connections_reached", "server");
                }
                crate::raven_bail!(crate::raven_error!(
                    max_connections_exceeded,
                    current,
                    self.max_connections,
                ));
            }
            match self.active_connections.compare_exchange(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(updated) => current = updated,
            }
        }

        let updated = current + 1;
        debug!("⟐ Active connections: {}", updated);

        // Update metrics
        if let Some(metrics) = metrics {
            metrics.record_connection("grpc");
            metrics.active_connections.set(updated as f64);
        }

        Ok(())
    }

    /// Decrement active connection count
    pub async fn decrement_connections(
        &self,
        duration: std::time::Duration,
        metrics: Option<&Arc<MetricsCollector>>,
    ) {
        let mut current = self.active_connections.load(Ordering::Relaxed);
        let new_count = loop {
            if current == 0 {
                break 0;
            }
            match self.active_connections.compare_exchange(
                current,
                current - 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => break current - 1,
                Err(updated) => current = updated,
            }
        };

        debug!("⟐ Active connections: {}", new_count);

        // Update metrics
        if let Some(metrics) = metrics {
            metrics.record_disconnection("grpc", duration);
            metrics.active_connections.set(new_count as f64);
        }
    }

    /// Get current active connection count
    pub async fn get_active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Get maximum connections limit
    pub fn get_max_connections(&self) -> usize {
        self.max_connections
    }
}

// Monitoring Module - Project Raven
// "The crows keep watch - comprehensive monitoring and observability"

pub mod health;
pub mod metrics;
pub mod tracing;

#[cfg(test)]
mod tests;

pub use health::*;
pub use metrics::*;
pub use tracing::*;

use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// Crow service that coordinates all observability components
pub struct CrowService {
    health_service: Arc<HealthService>,
    metrics_service: Arc<MetricsService>,
    tracing_service: Arc<TracingService>,
}

impl CrowService {
    /// Create a new crow service
    pub fn new(
        health_service: Arc<HealthService>,
        metrics_service: Arc<MetricsService>,
        tracing_service: Arc<TracingService>,
    ) -> Self {
        Self {
            health_service,
            metrics_service,
            tracing_service,
        }
    }

    /// Start all crow services
    pub async fn start(&self) -> Result<Vec<JoinHandle<()>>> {
        let mut handles = Vec::new();

        // Start health check service
        if let Some(handle) = self.health_service.start().await? {
            handles.push(handle);
        }

        // Start metrics service
        if let Some(handle) = self.metrics_service.start().await? {
            handles.push(handle);
        }

        // Initialize tracing
        self.tracing_service.initialize().await?;

        Ok(handles)
    }

    /// Get health service reference
    pub fn health(&self) -> &Arc<HealthService> {
        &self.health_service
    }

    /// Get metrics service reference
    pub fn metrics(&self) -> &Arc<MetricsService> {
        &self.metrics_service
    }

    /// Get tracing service reference
    pub fn tracing(&self) -> &Arc<TracingService> {
        &self.tracing_service
    }
}

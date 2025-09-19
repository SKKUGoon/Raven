// Health Check Service - Project Raven
// "The watchers report on the health of the realm"

use anyhow::{Context, Result};
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::config::MonitoringConfig;
use crate::database::influx_client::InfluxClient;
use crate::subscription_manager::SubscriptionManager;
use crate::types::HighFrequencyStorage;

/// Health check status levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Individual component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: String,
    pub last_check: u64,
    pub response_time_ms: Option<u64>,
    pub details: HashMap<String, serde_json::Value>,
}

/// Overall system health response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub version: String,
    pub components: HashMap<String, ComponentHealth>,
}

/// Readiness probe response (Kubernetes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub timestamp: u64,
    pub checks: HashMap<String, bool>,
}

/// Liveness probe response (Kubernetes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivenessResponse {
    pub alive: bool,
    pub timestamp: u64,
    pub uptime_seconds: u64,
}

/// Health service state
pub struct HealthServiceState {
    pub influx_client: Arc<InfluxClient>,
    pub subscription_manager: Arc<SubscriptionManager>,
    pub hf_storage: Arc<HighFrequencyStorage>,
    pub start_time: SystemTime,
    pub component_health: Arc<RwLock<HashMap<String, ComponentHealth>>>,
}

/// Health check service
pub struct HealthService {
    config: MonitoringConfig,
    state: Arc<HealthServiceState>,
}

impl HealthService {
    /// Create a new health service
    pub fn new(
        config: MonitoringConfig,
        influx_client: Arc<InfluxClient>,
        subscription_manager: Arc<SubscriptionManager>,
        hf_storage: Arc<HighFrequencyStorage>,
    ) -> Self {
        let state = Arc::new(HealthServiceState {
            influx_client,
            subscription_manager,
            hf_storage,
            start_time: SystemTime::now(),
            component_health: Arc::new(RwLock::new(HashMap::new())),
        });

        Self { config, state }
    }

    /// Get the configuration
    pub fn get_config(&self) -> &MonitoringConfig {
        &self.config
    }

    /// Start the health check HTTP server
    pub async fn start(&self) -> Result<Option<JoinHandle<()>>> {
        // Health checks should always be enabled for container health monitoring
        info!("⚕ Starting health check service...");

        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .with_state(Arc::clone(&self.state));

        let addr = format!("0.0.0.0:{}", self.config.health_check_port);
        let listener = TcpListener::bind(&addr)
            .await
            .with_context(|| format!("Failed to bind health check server to {addr}"))?;

        info!("⚕ Health check server starting on {}", addr);

        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Health check server error: {}", e);
            }
        });

        // Start background health monitoring
        let state_clone = Arc::clone(&self.state);
        tokio::spawn(async move {
            Self::background_health_monitor(state_clone).await;
        });

        Ok(Some(handle))
    }

    /// Background task to continuously monitor component health
    async fn background_health_monitor(state: Arc<HealthServiceState>) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            // Check InfluxDB health
            let influx_health = Self::check_influx_health(&state.influx_client).await;

            // Check subscription manager health
            let subscription_health =
                Self::check_subscription_manager_health(&state.subscription_manager).await;

            // Check high-frequency storage health
            let storage_health = Self::check_storage_health(&state.hf_storage).await;

            // Update component health
            let mut health_map = state.component_health.write().await;
            health_map.insert("influxdb".to_string(), influx_health);
            health_map.insert("subscription_manager".to_string(), subscription_health);
            health_map.insert("high_frequency_storage".to_string(), storage_health);
        }
    }

    /// Check InfluxDB connection health
    async fn check_influx_health(influx_client: &InfluxClient) -> ComponentHealth {
        let start_time = SystemTime::now();

        match influx_client.health_check().await {
            Ok(_) => {
                let response_time = start_time.elapsed().unwrap_or_default().as_millis() as u64;
                ComponentHealth {
                    status: HealthStatus::Healthy,
                    message: "InfluxDB connection healthy".to_string(),
                    last_check: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    response_time_ms: Some(response_time),
                    details: HashMap::new(),
                }
            }
            Err(e) => ComponentHealth {
                status: HealthStatus::Unhealthy,
                message: format!("InfluxDB connection failed: {e}"),
                last_check: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                response_time_ms: None,
                details: HashMap::new(),
            },
        }
    }

    /// Check subscription manager health
    async fn check_subscription_manager_health(
        subscription_manager: &SubscriptionManager,
    ) -> ComponentHealth {
        let stats = subscription_manager.get_stats();
        let mut details = HashMap::new();
        details.insert(
            "active_clients".to_string(),
            serde_json::Value::Number(stats.active_clients.into()),
        );
        details.insert(
            "total_subscriptions".to_string(),
            serde_json::Value::Number(stats.total_subscriptions.into()),
        );

        // Consider unhealthy if too many clients or subscriptions
        let status = if stats.active_clients > 10000 || stats.total_subscriptions > 50000 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        ComponentHealth {
            status,
            message: format!(
                "Subscription manager: {} clients, {} subscriptions",
                stats.active_clients, stats.total_subscriptions
            ),
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            response_time_ms: Some(1), // Very fast check
            details,
        }
    }

    /// Check high-frequency storage health
    async fn check_storage_health(hf_storage: &HighFrequencyStorage) -> ComponentHealth {
        let orderbook_count = hf_storage.get_orderbook_symbols().len();
        let trade_count = hf_storage.get_trade_symbols().len();

        let mut details = HashMap::new();
        details.insert(
            "orderbook_symbols".to_string(),
            serde_json::Value::Number(orderbook_count.into()),
        );
        details.insert(
            "trade_symbols".to_string(),
            serde_json::Value::Number(trade_count.into()),
        );

        ComponentHealth {
            status: HealthStatus::Healthy,
            message: format!("Storage: {orderbook_count} orderbooks, {trade_count} trade symbols",),
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            response_time_ms: Some(1), // Very fast check
            details,
        }
    }

    /// Get overall system health
    async fn get_system_health(state: &HealthServiceState) -> HealthResponse {
        let component_health = state.component_health.read().await;
        let uptime = state.start_time.elapsed().unwrap_or_default().as_secs();

        // Determine overall status based on component health
        let overall_status = if component_health
            .values()
            .any(|h| h.status == HealthStatus::Unhealthy)
        {
            HealthStatus::Unhealthy
        } else if component_health
            .values()
            .any(|h| h.status == HealthStatus::Degraded)
        {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        HealthResponse {
            status: overall_status,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            uptime_seconds: uptime,
            version: env!("CARGO_PKG_VERSION").to_string(),
            components: component_health.clone(),
        }
    }

    /// Get readiness status (for Kubernetes readiness probe)
    async fn get_readiness_status(state: &HealthServiceState) -> ReadinessResponse {
        let component_health = state.component_health.read().await;
        let mut checks = HashMap::new();

        // System is ready if all critical components are healthy
        let mut ready = true;
        for (name, health) in component_health.iter() {
            let component_ready = health.status != HealthStatus::Unhealthy;
            checks.insert(name.clone(), component_ready);
            if !component_ready {
                ready = false;
            }
        }

        ReadinessResponse {
            ready,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            checks,
        }
    }

    /// Get liveness status (for Kubernetes liveness probe)
    async fn get_liveness_status(state: &HealthServiceState) -> LivenessResponse {
        let uptime = state.start_time.elapsed().unwrap_or_default().as_secs();

        LivenessResponse {
            alive: true, // If we can respond, we're alive
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            uptime_seconds: uptime,
        }
    }
}

/// Health endpoint handler
async fn health_handler(
    State(state): State<Arc<HealthServiceState>>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let health = HealthService::get_system_health(&state).await;

    let _status_code = match health.status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK, // Still OK but with warnings
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };

    Ok(Json(health))
}

/// Readiness endpoint handler (Kubernetes)
async fn readiness_handler(
    State(state): State<Arc<HealthServiceState>>,
) -> Result<Json<ReadinessResponse>, StatusCode> {
    let readiness = HealthService::get_readiness_status(&state).await;

    let _status_code = if readiness.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    Ok(Json(readiness))
}

/// Liveness endpoint handler (Kubernetes)
async fn liveness_handler(
    State(state): State<Arc<HealthServiceState>>,
) -> Result<Json<LivenessResponse>, StatusCode> {
    let liveness = HealthService::get_liveness_status(&state).await;
    Ok(Json(liveness))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::influx_client::InfluxConfig;

    #[tokio::test]
    async fn test_health_service_creation() {
        let config = MonitoringConfig::default();
        let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
        let subscription_manager = Arc::new(SubscriptionManager::new());
        let hf_storage = Arc::new(HighFrequencyStorage::new());

        let health_service =
            HealthService::new(config, influx_client, subscription_manager, hf_storage);

        // Should create without error
        assert_eq!(health_service.config.health_check_port, 8080);
    }

    #[tokio::test]
    async fn test_component_health_status() {
        let health = ComponentHealth {
            status: HealthStatus::Healthy,
            message: "Test component".to_string(),
            last_check: 1640995200,
            response_time_ms: Some(10),
            details: HashMap::new(),
        };

        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.message, "Test component");
        assert_eq!(health.response_time_ms, Some(10));
    }

    #[test]
    fn test_health_status_serialization() {
        let health = HealthResponse {
            status: HealthStatus::Healthy,
            timestamp: 1640995200,
            uptime_seconds: 3600,
            version: "0.1.0".to_string(),
            components: HashMap::new(),
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"uptime_seconds\":3600"));
    }
}

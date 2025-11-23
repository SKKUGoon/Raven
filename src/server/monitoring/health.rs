// Health Check Service - Project Raven
// "The watchers report on the health of the realm"

use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::common::config::MonitoringConfig;
use crate::common::db::influx_client::InfluxClient;
use crate::common::error::RavenResult;
use crate::server::data_engine::storage::HighFrequencyStorage;
use crate::server::stream_router::StreamRouter;

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
    pub stream_router: Arc<StreamRouter>,
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
        stream_router: Arc<StreamRouter>,
        hf_storage: Arc<HighFrequencyStorage>,
    ) -> Self {
        let state = Arc::new(HealthServiceState {
            influx_client,
            stream_router,
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
    pub async fn start(&self) -> RavenResult<Option<JoinHandle<()>>> {
        // Health checks should always be enabled for container health monitoring
        info!("⚕ Starting health check service...");

        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .with_state(Arc::clone(&self.state));

        let addr = format!("0.0.0.0:{}", self.config.health_check_port);
        let listener = TcpListener::bind(&addr).await.map_err(|e| {
            crate::raven_error!(
                connection_error,
                format!("Failed to bind health check server to {addr}: {e}")
            )
        })?;

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

            // Check stream router health
            let router_health = Self::check_stream_router_health(&state.stream_router).await;

            // Check high-frequency storage health
            let storage_health = Self::check_storage_health(&state.hf_storage).await;

            // Update component health
            let mut health_map = state.component_health.write().await;
            health_map.insert("influxdb".to_string(), influx_health);
            health_map.insert("stream_router".to_string(), router_health);
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

    /// Check stream router health
    async fn check_stream_router_health(stream_router: &StreamRouter) -> ComponentHealth {
        let stats = stream_router.get_stats();
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
                "Stream router: {} clients, {} subscriptions",
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

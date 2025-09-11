// Metrics Service - Project Raven
// "Counting every raven that flies, every message that's carried"

use anyhow::{Context, Result};
use axum::{extract::State, http::StatusCode, response::Response, routing::get, Router};
use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry,
    TextEncoder,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::config::MonitoringConfig;

/// Prometheus metrics collector for the market data server
pub struct MetricsCollector {
    // Connection metrics
    pub active_connections: Gauge,
    pub total_connections: Counter,
    pub connection_duration: HistogramVec,

    // gRPC metrics
    pub grpc_requests_total: CounterVec,
    pub grpc_request_duration: HistogramVec,
    pub grpc_streaming_connections: Gauge,

    // Subscription metrics
    pub active_subscriptions: Gauge,
    pub subscription_operations: CounterVec,
    pub subscription_duration: HistogramVec,

    // Data processing metrics
    pub messages_processed: CounterVec,
    pub message_processing_duration: HistogramVec,
    pub atomic_operations: CounterVec,
    pub atomic_operation_duration: HistogramVec,

    // Database metrics
    pub database_operations: CounterVec,
    pub database_operation_duration: HistogramVec,
    pub database_connection_pool: GaugeVec,

    // System metrics
    pub memory_usage: Gauge,
    pub cpu_usage: Gauge,
    pub uptime_seconds: Gauge,

    // Error metrics
    pub errors_total: CounterVec,
    pub circuit_breaker_state: GaugeVec,

    // Performance metrics
    pub throughput_messages_per_second: Gauge,
    pub latency_percentiles: HistogramVec,
}

/// Metrics service that exposes Prometheus metrics
pub struct MetricsService {
    config: MonitoringConfig,
    registry: Arc<Registry>,
    collector: Arc<MetricsCollector>,
    start_time: Instant,
}

impl MetricsService {
    /// Create a new metrics service
    pub fn new(config: MonitoringConfig) -> Result<Self> {
        let registry = Arc::new(Registry::new());
        let collector = Arc::new(MetricsCollector::new()?);

        // Register all metrics with the registry
        collector.register(&registry)?;

        Ok(Self {
            config,
            registry,
            collector,
            start_time: Instant::now(),
        })
    }

    /// Start the metrics HTTP server
    pub async fn start(&self) -> Result<Option<JoinHandle<()>>> {
        if !self.config.metrics_enabled {
            info!("📊 Metrics collection disabled in configuration");
            return Ok(None);
        }

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(Arc::clone(&self.registry));

        let addr = format!("0.0.0.0:{}", self.config.metrics_port);
        let listener = TcpListener::bind(&addr)
            .await
            .with_context(|| format!("Failed to bind metrics server to {addr}"))?;

        info!("📊 Metrics server starting on {}", addr);

        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Metrics server error: {}", e);
            }
        });

        // Start background metrics collection
        let collector_clone = Arc::clone(&self.collector);
        let start_time = self.start_time;
        tokio::spawn(async move {
            Self::background_metrics_collection(collector_clone, start_time).await;
        });

        Ok(Some(handle))
    }

    /// Get metrics collector reference
    pub fn collector(&self) -> &Arc<MetricsCollector> {
        &self.collector
    }

    /// Background task to collect system metrics
    async fn background_metrics_collection(collector: Arc<MetricsCollector>, start_time: Instant) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            // Update uptime
            let uptime = start_time.elapsed().as_secs() as f64;
            collector.uptime_seconds.set(uptime);

            // Collect system metrics (simplified - in production you'd use proper system monitoring)
            if let Ok(memory_info) = Self::get_memory_usage() {
                collector.memory_usage.set(memory_info);
            }

            if let Ok(cpu_info) = Self::get_cpu_usage() {
                collector.cpu_usage.set(cpu_info);
            }
        }
    }

    /// Get current memory usage (simplified implementation)
    fn get_memory_usage() -> Result<f64> {
        // In a real implementation, you'd use a proper system monitoring library
        // For now, return a placeholder value
        Ok(0.0)
    }

    /// Get current CPU usage (simplified implementation)
    fn get_cpu_usage() -> Result<f64> {
        // In a real implementation, you'd use a proper system monitoring library
        // For now, return a placeholder value
        Ok(0.0)
    }
}

impl MetricsCollector {
    /// Create a new metrics collector with all metrics initialized
    pub fn new() -> Result<Self> {
        Ok(Self {
            // Connection metrics
            active_connections: Gauge::new(
                "raven_active_connections",
                "Number of active client connections",
            )?,
            total_connections: Counter::new(
                "raven_total_connections",
                "Total number of client connections",
            )?,
            connection_duration: HistogramVec::new(
                HistogramOpts::new(
                    "raven_connection_duration_seconds",
                    "Duration of client connections",
                ),
                &["client_type"],
            )?,

            // gRPC metrics
            grpc_requests_total: CounterVec::new(
                Opts::new("raven_grpc_requests_total", "Total gRPC requests"),
                &["method", "status"],
            )?,
            grpc_request_duration: HistogramVec::new(
                HistogramOpts::new(
                    "raven_grpc_request_duration_seconds",
                    "gRPC request duration",
                ),
                &["method"],
            )?,
            grpc_streaming_connections: Gauge::new(
                "raven_grpc_streaming_connections",
                "Number of active gRPC streaming connections",
            )?,

            // Subscription metrics
            active_subscriptions: Gauge::new(
                "raven_active_subscriptions",
                "Number of active subscriptions",
            )?,
            subscription_operations: CounterVec::new(
                Opts::new(
                    "raven_subscription_operations_total",
                    "Subscription operations",
                ),
                &["operation", "data_type"],
            )?,
            subscription_duration: HistogramVec::new(
                HistogramOpts::new(
                    "raven_subscription_duration_seconds",
                    "Subscription operation duration",
                ),
                &["operation"],
            )?,

            // Data processing metrics
            messages_processed: CounterVec::new(
                Opts::new("raven_messages_processed_total", "Messages processed"),
                &["data_type", "source"],
            )?,
            message_processing_duration: HistogramVec::new(
                HistogramOpts::new(
                    "raven_message_processing_duration_seconds",
                    "Message processing duration",
                )
                .buckets(vec![
                    0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0,
                ]),
                &["data_type"],
            )?,
            atomic_operations: CounterVec::new(
                Opts::new("raven_atomic_operations_total", "Atomic operations"),
                &["operation_type", "symbol"],
            )?,
            atomic_operation_duration: HistogramVec::new(
                HistogramOpts::new(
                    "raven_atomic_operation_duration_seconds",
                    "Atomic operation duration",
                )
                .buckets(vec![
                    0.000001, 0.000005, 0.00001, 0.00005, 0.0001, 0.0005, 0.001,
                ]),
                &["operation_type"],
            )?,

            // Database metrics
            database_operations: CounterVec::new(
                Opts::new("raven_database_operations_total", "Database operations"),
                &["operation", "measurement", "status"],
            )?,
            database_operation_duration: HistogramVec::new(
                HistogramOpts::new(
                    "raven_database_operation_duration_seconds",
                    "Database operation duration",
                ),
                &["operation", "measurement"],
            )?,
            database_connection_pool: GaugeVec::new(
                Opts::new(
                    "raven_database_connection_pool",
                    "Database connection pool status",
                ),
                &["status"],
            )?,

            // System metrics
            memory_usage: Gauge::new("raven_memory_usage_bytes", "Memory usage in bytes")?,
            cpu_usage: Gauge::new("raven_cpu_usage_percent", "CPU usage percentage")?,
            uptime_seconds: Gauge::new("raven_uptime_seconds", "Server uptime in seconds")?,

            // Error metrics
            errors_total: CounterVec::new(
                Opts::new("raven_errors_total", "Total errors"),
                &["error_type", "component"],
            )?,
            circuit_breaker_state: GaugeVec::new(
                Opts::new(
                    "raven_circuit_breaker_state",
                    "Circuit breaker state (0=closed, 1=open, 2=half-open)",
                ),
                &["component"],
            )?,

            // Performance metrics
            throughput_messages_per_second: Gauge::new(
                "raven_throughput_messages_per_second",
                "Current message throughput",
            )?,
            latency_percentiles: HistogramVec::new(
                HistogramOpts::new("raven_latency_percentiles_seconds", "Latency percentiles")
                    .buckets(vec![
                        0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0,
                    ]),
                &["operation"],
            )?,
        })
    }

    /// Register all metrics with the Prometheus registry
    pub fn register(&self, registry: &Registry) -> Result<()> {
        // Connection metrics
        registry.register(Box::new(self.active_connections.clone()))?;
        registry.register(Box::new(self.total_connections.clone()))?;
        registry.register(Box::new(self.connection_duration.clone()))?;

        // gRPC metrics
        registry.register(Box::new(self.grpc_requests_total.clone()))?;
        registry.register(Box::new(self.grpc_request_duration.clone()))?;
        registry.register(Box::new(self.grpc_streaming_connections.clone()))?;

        // Subscription metrics
        registry.register(Box::new(self.active_subscriptions.clone()))?;
        registry.register(Box::new(self.subscription_operations.clone()))?;
        registry.register(Box::new(self.subscription_duration.clone()))?;

        // Data processing metrics
        registry.register(Box::new(self.messages_processed.clone()))?;
        registry.register(Box::new(self.message_processing_duration.clone()))?;
        registry.register(Box::new(self.atomic_operations.clone()))?;
        registry.register(Box::new(self.atomic_operation_duration.clone()))?;

        // Database metrics
        registry.register(Box::new(self.database_operations.clone()))?;
        registry.register(Box::new(self.database_operation_duration.clone()))?;
        registry.register(Box::new(self.database_connection_pool.clone()))?;

        // System metrics
        registry.register(Box::new(self.memory_usage.clone()))?;
        registry.register(Box::new(self.cpu_usage.clone()))?;
        registry.register(Box::new(self.uptime_seconds.clone()))?;

        // Error metrics
        registry.register(Box::new(self.errors_total.clone()))?;
        registry.register(Box::new(self.circuit_breaker_state.clone()))?;

        // Performance metrics
        registry.register(Box::new(self.throughput_messages_per_second.clone()))?;
        registry.register(Box::new(self.latency_percentiles.clone()))?;

        Ok(())
    }

    /// Record a connection event
    pub fn record_connection(&self, _client_type: &str) {
        self.total_connections.inc();
        self.active_connections.inc();
    }

    /// Record a disconnection event
    pub fn record_disconnection(&self, client_type: &str, duration: Duration) {
        self.active_connections.dec();
        self.connection_duration
            .with_label_values(&[client_type])
            .observe(duration.as_secs_f64());
    }

    /// Record a gRPC request
    pub fn record_grpc_request(&self, method: &str, status: &str, duration: Duration) {
        self.grpc_requests_total
            .with_label_values(&[method, status])
            .inc();
        self.grpc_request_duration
            .with_label_values(&[method])
            .observe(duration.as_secs_f64());
    }

    /// Record a subscription operation
    pub fn record_subscription_operation(
        &self,
        operation: &str,
        data_type: &str,
        duration: Duration,
    ) {
        self.subscription_operations
            .with_label_values(&[operation, data_type])
            .inc();
        self.subscription_duration
            .with_label_values(&[operation])
            .observe(duration.as_secs_f64());
    }

    /// Record message processing
    pub fn record_message_processed(&self, data_type: &str, source: &str, duration: Duration) {
        self.messages_processed
            .with_label_values(&[data_type, source])
            .inc();
        self.message_processing_duration
            .with_label_values(&[data_type])
            .observe(duration.as_secs_f64());
    }

    /// Record atomic operation
    pub fn record_atomic_operation(&self, operation_type: &str, symbol: &str, duration: Duration) {
        self.atomic_operations
            .with_label_values(&[operation_type, symbol])
            .inc();
        self.atomic_operation_duration
            .with_label_values(&[operation_type])
            .observe(duration.as_secs_f64());
    }

    /// Record database operation
    pub fn record_database_operation(
        &self,
        operation: &str,
        measurement: &str,
        status: &str,
        duration: Duration,
    ) {
        self.database_operations
            .with_label_values(&[operation, measurement, status])
            .inc();
        self.database_operation_duration
            .with_label_values(&[operation, measurement])
            .observe(duration.as_secs_f64());
    }

    /// Record error
    pub fn record_error(&self, error_type: &str, component: &str) {
        self.errors_total
            .with_label_values(&[error_type, component])
            .inc();
    }

    /// Update circuit breaker state
    pub fn update_circuit_breaker_state(&self, component: &str, state: f64) {
        self.circuit_breaker_state
            .with_label_values(&[component])
            .set(state);
    }

    /// Update throughput
    pub fn update_throughput(&self, messages_per_second: f64) {
        self.throughput_messages_per_second.set(messages_per_second);
    }

    /// Record latency
    pub fn record_latency(&self, operation: &str, duration: Duration) {
        self.latency_percentiles
            .with_label_values(&[operation])
            .observe(duration.as_secs_f64());
    }
}

/// Metrics endpoint handler
async fn metrics_handler(
    State(registry): State<Arc<Registry>>,
) -> Result<Response<String>, StatusCode> {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();

    match encoder.encode_to_string(&metric_families) {
        Ok(output) => {
            let response = Response::builder()
                .header("Content-Type", encoder.format_type())
                .body(output)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(response)
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new().unwrap();

        // Test that metrics can be recorded
        collector.record_connection("test");
        collector.record_disconnection("test", Duration::from_secs(1));

        // Should not panic
    }

    #[tokio::test]
    async fn test_metrics_service_creation() {
        let config = MonitoringConfig::default();
        let service = MetricsService::new(config).unwrap();

        // Should create without error
        assert!(service.collector.active_connections.get() >= 0.0);
    }

    #[test]
    fn test_metrics_registration() {
        let registry = Registry::new();
        let collector = MetricsCollector::new().unwrap();

        // Should register without error
        collector.register(&registry).unwrap();

        // Should have metrics registered
        let families = registry.gather();
        assert!(!families.is_empty());
    }
}

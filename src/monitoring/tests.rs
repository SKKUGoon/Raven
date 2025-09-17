// Monitoring Integration Tests - Project Raven
// "Testing the crows who watch the watchers"

use crate::config::MonitoringConfig;
use crate::database::influx_client::{InfluxClient, InfluxConfig};
use crate::monitoring::{
    CrowService, HealthService, MetricsCollector, MetricsService, PerformanceSpan, TracingService,
    TracingUtils,
};
use crate::subscription_manager::SubscriptionManager;
use crate::types::HighFrequencyStorage;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_monitoring_service_integration() {
    let config = MonitoringConfig {
        metrics_enabled: false, // Disable to avoid port conflicts in tests
        health_check_port: 0,   // Let OS assign an ephemeral port
        metrics_port: 0,
        tracing_enabled: false,
        log_level: "debug".to_string(),
        performance_monitoring: true,
    };

    // Create dependencies
    let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
    let subscription_manager = Arc::new(SubscriptionManager::new());
    let hf_storage = Arc::new(HighFrequencyStorage::new());

    // Create services
    let health_service = Arc::new(HealthService::new(
        config.clone(),
        influx_client,
        subscription_manager,
        hf_storage,
    ));

    let metrics_service = Arc::new(MetricsService::new(config.clone()).unwrap());
    let tracing_service = Arc::new(TracingService::new(config.clone()));

    // Create monitoring service
    let monitoring_service = CrowService::new(health_service, metrics_service, tracing_service);

    match monitoring_service.start().await {
        Ok(handles) => assert_eq!(handles.len(), 1),
        Err(e) => assert!(
            e.to_string().contains("Failed to bind health check server"),
            "unexpected error: {e}"
        ),
    }
}

#[tokio::test]
async fn test_metrics_collection() {
    let config = MonitoringConfig::default();
    let service = MetricsService::new(config).unwrap();
    let collector = service.collector();

    // Test connection metrics
    collector.record_connection("test_client");
    collector.record_disconnection("test_client", Duration::from_secs(1));

    // Test gRPC metrics
    collector.record_grpc_request("StreamMarketData", "success", Duration::from_millis(10));
    collector.record_grpc_request("Subscribe", "error", Duration::from_millis(5));

    // Test subscription metrics
    collector.record_subscription_operation("subscribe", "orderbook", Duration::from_millis(2));
    collector.record_subscription_operation("unsubscribe", "trades", Duration::from_millis(1));

    // Test message processing metrics
    collector.record_message_processed("orderbook", "websocket", Duration::from_micros(500));
    collector.record_message_processed("trade", "websocket", Duration::from_micros(300));

    // Test atomic operations
    collector.record_atomic_operation("update", "BTCUSDT", Duration::from_nanos(1000));
    collector.record_atomic_operation("read", "ETHUSDT", Duration::from_nanos(500));

    // Test database operations
    collector.record_database_operation("write", "orderbook", "success", Duration::from_millis(5));
    collector.record_database_operation("query", "trades", "error", Duration::from_millis(10));

    // Test error recording
    collector.record_error("connection_timeout", "grpc");
    collector.record_error("database_error", "influxdb");

    // Test circuit breaker state
    collector.update_circuit_breaker_state("database", 0.0); // Closed
    collector.update_circuit_breaker_state("grpc", 1.0); // Open

    // Test throughput and latency
    collector.update_throughput(1000.0);
    collector.record_latency("end_to_end", Duration::from_millis(1));

    // All operations should complete without error
}

#[tokio::test]
async fn test_health_service_components() {
    let config = MonitoringConfig {
        metrics_enabled: false,
        ..Default::default()
    };

    let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
    let subscription_manager = Arc::new(SubscriptionManager::new());
    let hf_storage = Arc::new(HighFrequencyStorage::new());

    let health_service =
        HealthService::new(config, influx_client, subscription_manager, hf_storage);

    // Should create without error
    assert_eq!(health_service.get_config().health_check_port, 8080);
}

#[tokio::test]
async fn test_tracing_service_initialization() {
    let config = MonitoringConfig {
        tracing_enabled: false, // Disable to avoid OpenTelemetry setup in tests
        log_level: "debug".to_string(),
        ..Default::default()
    };

    let service = TracingService::new(config);

    // Should initialize basic logging without error
    service.initialize().await.unwrap();

    // Should shutdown without error
    service.shutdown().await.unwrap();
}

#[test]
fn test_performance_span() {
    let span = PerformanceSpan::new("test_operation");

    // Simulate some work
    std::thread::sleep(Duration::from_millis(1));

    // Complete successfully
    span.complete(true);
}

#[test]
fn test_performance_span_error() {
    let span = PerformanceSpan::new("failing_operation");

    // Simulate error
    span.error("Test error occurred");
}

#[test]
fn test_tracing_utils() {
    // Test span creation
    let _grpc_span = TracingUtils::grpc_span("StreamMarketData", "client123");
    let _db_span = TracingUtils::database_span("write", "orderbook");
    let _atomic_span = TracingUtils::atomic_span("update", "BTCUSDT");
    let _sub_span = TracingUtils::subscription_span(
        "subscribe",
        "client123",
        &["BTCUSDT".to_string(), "ETHUSDT".to_string()],
    );

    // Test event recording
    TracingUtils::record_event(
        "test_event",
        &[("key1", &"value1"), ("key2", &"42"), ("key3", &"true")],
    );

    // Test performance recording
    TracingUtils::record_atomic_performance("update", "BTCUSDT", 1000);
    TracingUtils::record_throughput(1500.0, "orderbook");
    TracingUtils::record_latency_percentile("grpc_request", 95.0, 2.5);

    // Should not panic
}

#[tokio::test]
async fn test_health_check_responses() {
    use crate::monitoring::health::*;

    // Test health status serialization
    let health = HealthResponse {
        status: HealthStatus::Healthy,
        timestamp: 1640995200,
        uptime_seconds: 3600,
        version: "0.1.0".to_string(),
        components: std::collections::HashMap::new(),
    };

    let json = serde_json::to_string(&health).unwrap();
    assert!(json.contains("\"status\":\"healthy\""));

    // Test readiness response
    let readiness = ReadinessResponse {
        ready: true,
        timestamp: 1640995200,
        checks: std::collections::HashMap::new(),
    };

    let json = serde_json::to_string(&readiness).unwrap();
    assert!(json.contains("\"ready\":true"));

    // Test liveness response
    let liveness = LivenessResponse {
        alive: true,
        timestamp: 1640995200,
        uptime_seconds: 3600,
    };

    let json = serde_json::to_string(&liveness).unwrap();
    assert!(json.contains("\"alive\":true"));
}

#[tokio::test]
async fn test_component_health_checks() {
    use crate::monitoring::health::*;

    // Test component health creation
    let mut details = std::collections::HashMap::new();
    details.insert(
        "connections".to_string(),
        serde_json::Value::Number(10.into()),
    );

    let health = ComponentHealth {
        status: HealthStatus::Healthy,
        message: "Component is healthy".to_string(),
        last_check: 1640995200,
        response_time_ms: Some(5),
        details,
    };

    assert_eq!(health.status, HealthStatus::Healthy);
    assert_eq!(health.response_time_ms, Some(5));
    assert!(health.details.contains_key("connections"));
}

#[test]
fn test_metrics_registration() {
    use prometheus::Registry;

    let registry = Registry::new();
    let collector = MetricsCollector::new().unwrap();

    // Should register without error
    collector.register(&registry).unwrap();

    // Should have metrics registered
    let families = registry.gather();
    assert!(!families.is_empty());

    // Check for some expected metrics
    let metric_names: Vec<String> = families.iter().map(|f| f.name().to_string()).collect();

    // Check that we have some metrics registered
    assert!(!metric_names.is_empty());
    assert!(metric_names.contains(&"raven_active_connections".to_string()));
}

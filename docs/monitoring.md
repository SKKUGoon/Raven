# Monitoring and Observability - Project Raven

> "The watchers must be watched - comprehensive monitoring for the realm"

This document describes the monitoring and observability features implemented in Project Raven, providing comprehensive insights into system health, performance, and behavior.

## Overview

Project Raven includes a complete monitoring stack with:

- **Health Checks**: Kubernetes-compatible readiness and liveness probes
- **Metrics Collection**: Prometheus metrics for throughput, latency, and errors
- **Distributed Tracing**: OpenTelemetry integration for request tracing
- **Performance Monitoring**: Atomic operation performance tracking
- **Alerting**: Critical error and performance degradation detection

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Monitoring Services                       │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Health    │  │   Metrics   │  │   Distributed       │  │
│  │   Service   │  │   Service   │  │   Tracing           │  │
│  │             │  │             │  │                     │  │
│  │ K8s Probes  │  │ Prometheus  │  │ OpenTelemetry       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                 Application Components                      │
├─────────────────────────────────────────────────────────────┤
│  gRPC Server  │  Subscription Mgr  │  InfluxDB  │  Storage │
│               │                    │            │          │
│  Metrics ●────┼────● Metrics ●─────┼───● Health │  ● Stats │
│  Tracing ●────┼────● Tracing ●─────┼───● Checks │  ● Perf  │
└─────────────────────────────────────────────────────────────┘
```

## Health Checks

### Endpoints

The health service provides three endpoints for different monitoring needs:

#### `/health` - Overall System Health
```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "timestamp": 1640995200,
  "uptime_seconds": 3600,
  "version": "0.1.0",
  "components": {
    "influxdb": {
      "status": "healthy",
      "message": "InfluxDB connection healthy",
      "last_check": 1640995200,
      "response_time_ms": 5,
      "details": {}
    },
    "subscription_manager": {
      "status": "healthy",
      "message": "Subscription manager: 150 clients, 450 subscriptions",
      "last_check": 1640995200,
      "response_time_ms": 1,
      "details": {
        "active_clients": 150,
        "total_subscriptions": 450
      }
    },
    "high_frequency_storage": {
      "status": "healthy",
      "message": "Storage: 25 orderbooks, 25 trade symbols",
      "last_check": 1640995200,
      "response_time_ms": 1,
      "details": {
        "orderbook_symbols": 25,
        "trade_symbols": 25
      }
    }
  }
}
```

#### `/health/ready` - Kubernetes Readiness Probe
```bash
curl http://localhost:8080/health/ready
```

Response:
```json
{
  "ready": true,
  "timestamp": 1640995200,
  "checks": {
    "influxdb": true,
    "subscription_manager": true,
    "high_frequency_storage": true
  }
}
```

#### `/health/live` - Kubernetes Liveness Probe
```bash
curl http://localhost:8080/health/live
```

Response:
```json
{
  "alive": true,
  "timestamp": 1640995200,
  "uptime_seconds": 3600
}
```

### Health Status Levels

- **Healthy**: All systems operating normally
- **Degraded**: System functional but with warnings (e.g., high load)
- **Unhealthy**: Critical issues requiring attention

### Component Health Checks

1. **InfluxDB**: Connection health and response time
2. **Subscription Manager**: Client count and subscription load
3. **High-Frequency Storage**: Symbol count and memory usage

## Metrics Collection

### Prometheus Integration

Metrics are exposed at `/metrics` endpoint in Prometheus format:

```bash
curl http://localhost:9090/metrics
```

### Available Metrics

#### Connection Metrics
- `raven_active_connections`: Current active client connections
- `raven_total_connections`: Total connections since startup
- `raven_connection_duration_seconds`: Connection duration histogram

#### gRPC Metrics
- `raven_grpc_requests_total`: Total gRPC requests by method and status
- `raven_grpc_request_duration_seconds`: gRPC request duration histogram
- `raven_grpc_streaming_connections`: Active streaming connections

#### Subscription Metrics
- `raven_active_subscriptions`: Current active subscriptions
- `raven_subscription_operations_total`: Subscription operations by type
- `raven_subscription_duration_seconds`: Subscription operation duration

#### Data Processing Metrics
- `raven_messages_processed_total`: Messages processed by data type and source
- `raven_message_processing_duration_seconds`: Message processing duration
- `raven_atomic_operations_total`: Atomic operations by type and symbol
- `raven_atomic_operation_duration_seconds`: Atomic operation duration (sub-microsecond)

#### Database Metrics
- `raven_database_operations_total`: Database operations by type and status
- `raven_database_operation_duration_seconds`: Database operation duration
- `raven_database_connection_pool`: Connection pool status

#### System Metrics
- `raven_memory_usage_bytes`: Memory usage
- `raven_cpu_usage_percent`: CPU usage percentage
- `raven_uptime_seconds`: Server uptime

#### Error Metrics
- `raven_errors_total`: Total errors by type and component
- `raven_circuit_breaker_state`: Circuit breaker state (0=closed, 1=open, 2=half-open)

#### Performance Metrics
- `raven_throughput_messages_per_second`: Current message throughput
- `raven_latency_percentiles_seconds`: Latency percentiles by operation

### Example Queries

```promql
# Average request rate over 5 minutes
rate(raven_grpc_requests_total[5m])

# 95th percentile latency
histogram_quantile(0.95, raven_grpc_request_duration_seconds_bucket)

# Error rate by component
rate(raven_errors_total[5m])

# Atomic operation performance
histogram_quantile(0.99, raven_atomic_operation_duration_seconds_bucket)
```

## Distributed Tracing

### OpenTelemetry Integration

Project Raven integrates with OpenTelemetry for distributed tracing:

```rust
use market_data_subscription_server::monitoring::TracingUtils;

// Create spans for operations
let span = TracingUtils::grpc_span("StreamMarketData", "client123");
let db_span = TracingUtils::database_span("write", "orderbook");

// Record custom events
TracingUtils::record_event("subscription_created", &[
    ("client_id", &"client123"),
    ("symbol", &"BTCUSDT"),
    ("data_type", &"orderbook"),
]);
```

### Trace Context

Traces include:
- **Operation Type**: gRPC, database, atomic, subscription
- **Duration**: Precise timing information
- **Success/Failure**: Operation outcome
- **Custom Attributes**: Symbol, client ID, data type, etc.

### Performance Spans

For critical operations, use performance spans:

```rust
use market_data_subscription_server::monitoring::PerformanceSpan;

let span = PerformanceSpan::new("critical_operation");
// ... perform operation ...
span.complete(true); // or span.error("error message")
```

## Configuration

### Environment Variables

```bash
# Monitoring Configuration
RAVEN_MONITORING__METRICS_ENABLED=true
RAVEN_MONITORING__METRICS_PORT=9090
RAVEN_MONITORING__HEALTH_CHECK_PORT=8080
RAVEN_MONITORING__TRACING_ENABLED=true
RAVEN_MONITORING__LOG_LEVEL=info
RAVEN_MONITORING__PERFORMANCE_MONITORING=true

# OpenTelemetry Configuration
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_SERVICE_NAME=market-data-subscription-server
OTEL_SERVICE_VERSION=0.1.0
```

### Configuration File

```toml
[monitoring]
metrics_enabled = true
metrics_port = 9090
health_check_port = 8080
tracing_enabled = true
log_level = "info"
performance_monitoring = true
```

## Kubernetes Integration

### Deployment Configuration

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: raven-server
spec:
  template:
    spec:
      containers:
      - name: raven
        image: raven:latest
        ports:
        - containerPort: 50051  # gRPC
        - containerPort: 8080   # Health checks
        - containerPort: 9090   # Metrics
        
        # Health checks
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
          
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

### Service Monitor (Prometheus Operator)

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: raven-metrics
spec:
  selector:
    matchLabels:
      app: raven-server
  endpoints:
  - port: metrics
    interval: 15s
    path: /metrics
```

## Alerting Rules

### Prometheus Alerting Rules

```yaml
groups:
- name: raven.rules
  rules:
  - alert: RavenHighErrorRate
    expr: rate(raven_errors_total[5m]) > 0.1
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High error rate in Raven server"
      
  - alert: RavenDatabaseDown
    expr: raven_circuit_breaker_state{component="database"} == 1
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Database circuit breaker is open"
      
  - alert: RavenHighLatency
    expr: histogram_quantile(0.95, raven_grpc_request_duration_seconds_bucket) > 0.1
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High gRPC request latency"
      
  - alert: RavenAtomicOperationSlow
    expr: histogram_quantile(0.99, raven_atomic_operation_duration_seconds_bucket) > 0.001
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "Atomic operations are slower than expected"
```

## Performance Monitoring

### Atomic Operation Monitoring

Project Raven provides sub-microsecond monitoring of atomic operations:

```rust
// Automatic instrumentation
collector.record_atomic_operation("update", "BTCUSDT", Duration::from_nanos(750));

// Manual tracing
TracingUtils::record_atomic_performance("update", "BTCUSDT", 750);
```

### Throughput Monitoring

Real-time throughput tracking:

```rust
collector.update_throughput(messages_per_second);
TracingUtils::record_throughput(2000.0, "orderbook");
```

### Latency Percentiles

Track latency percentiles for different operations:

```rust
collector.record_latency("end_to_end", duration);
TracingUtils::record_latency_percentile("grpc_request", 95.0, latency_ms);
```

## Grafana Dashboards

### Key Metrics Dashboard

1. **System Overview**
   - Active connections
   - Request rate
   - Error rate
   - Uptime

2. **Performance Metrics**
   - Latency percentiles
   - Throughput
   - Atomic operation performance
   - Database operation timing

3. **Component Health**
   - Circuit breaker states
   - Connection pool status
   - Subscription statistics

### Example Grafana Queries

```promql
# Connection rate
rate(raven_total_connections[5m])

# Message processing rate by type
sum(rate(raven_messages_processed_total[5m])) by (data_type)

# Database operation success rate
sum(rate(raven_database_operations_total{status="success"}[5m])) / 
sum(rate(raven_database_operations_total[5m]))
```

## Troubleshooting

### Common Issues

1. **High Memory Usage**
   - Check `raven_memory_usage_bytes`
   - Monitor subscription count
   - Review buffer sizes

2. **Slow Atomic Operations**
   - Check `raven_atomic_operation_duration_seconds`
   - Monitor CPU usage
   - Review cache alignment

3. **Database Connection Issues**
   - Check `raven_circuit_breaker_state{component="database"}`
   - Monitor `raven_database_connection_pool`
   - Review connection timeouts

### Debug Mode

Enable debug logging for detailed monitoring information:

```bash
RAVEN_MONITORING__LOG_LEVEL=debug
```

## Best Practices

1. **Metrics Collection**
   - Use appropriate bucket sizes for histograms
   - Avoid high-cardinality labels
   - Monitor metric collection overhead

2. **Health Checks**
   - Keep health checks lightweight
   - Set appropriate timeouts
   - Monitor health check response times

3. **Tracing**
   - Use sampling in production
   - Include relevant context in spans
   - Monitor trace collection overhead

4. **Alerting**
   - Set meaningful thresholds
   - Avoid alert fatigue
   - Include actionable information

## Example Usage

See `examples/monitoring_example.rs` for a complete demonstration of the monitoring system in action.

```bash
cargo run --example monitoring_example
```

This example shows:
- Service initialization
- Metrics collection
- Health check endpoints
- Distributed tracing
- Performance monitoring

The monitoring system provides comprehensive observability into Project Raven's operation, enabling effective monitoring, debugging, and performance optimization in production environments.
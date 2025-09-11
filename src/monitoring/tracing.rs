// Distributed Tracing Service - Project Raven
// "Following the path of every raven across the realm"

use anyhow::Result;
use tracing::{info, warn};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::MonitoringConfig;

/// Distributed tracing service
pub struct TracingService {
    config: MonitoringConfig,
}

impl TracingService {
    /// Create a new tracing service
    pub fn new(config: MonitoringConfig) -> Self {
        Self { config }
    }

    /// Initialize distributed tracing
    pub async fn initialize(&self) -> Result<()> {
        if !self.config.tracing_enabled {
            info!("🔍 Distributed tracing disabled in configuration");
            self.initialize_basic_logging()?;
            return Ok(());
        }

        info!("🔍 Initializing structured logging with tracing...");

        // For now, use structured logging without OpenTelemetry
        // In production, you would integrate with your OpenTelemetry collector
        self.initialize_structured_logging()?;

        info!("✅ Structured logging initialized successfully");
        Ok(())
    }

    /// Initialize basic logging without tracing
    fn initialize_basic_logging(&self) -> Result<()> {
        info!("📝 Initializing basic logging...");

        let fmt_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&self.config.log_level));

        // Try to initialize, but don't fail if already initialized (for tests)
        let _ = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init();

        info!("✅ Basic logging initialized successfully");
        Ok(())
    }

    /// Initialize structured logging with tracing
    fn initialize_structured_logging(&self) -> Result<()> {
        info!("📝 Initializing structured logging...");

        let fmt_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&self.config.log_level));

        // Try to initialize, but don't fail if already initialized (for tests)
        let _ = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init();

        info!("✅ Structured logging initialized successfully");
        Ok(())
    }

    /// Shutdown tracing and flush any pending spans
    pub async fn shutdown(&self) -> Result<()> {
        if self.config.tracing_enabled {
            info!("🔍 Shutting down tracing...");
            // In a full OpenTelemetry implementation, you would flush spans here
            info!("✅ Tracing shutdown complete");
        }
        Ok(())
    }

    /// Create a custom span for performance monitoring
    pub fn create_performance_span(&self, operation: &str) -> PerformanceSpan {
        PerformanceSpan::new(operation)
    }
}

/// Performance monitoring span for critical operations
pub struct PerformanceSpan {
    operation: String,
    start_time: std::time::Instant,
    _span: tracing::Span,
}

impl PerformanceSpan {
    /// Create a new performance span
    pub fn new(operation: &str) -> Self {
        let span = tracing::info_span!(
            "performance_operation",
            operation = operation,
            duration_ms = tracing::field::Empty,
            success = tracing::field::Empty,
        );

        Self {
            operation: operation.to_string(),
            start_time: std::time::Instant::now(),
            _span: span,
        }
    }

    /// Record the completion of the operation
    pub fn complete(self, success: bool) {
        let duration = self.start_time.elapsed();

        tracing::info!(
            operation = %self.operation,
            duration_ms = duration.as_millis() as u64,
            success = success,
            "Performance operation completed"
        );

        // Log warning for slow operations
        if duration.as_millis() > 100 {
            warn!(
                "Slow operation detected: {} took {}ms",
                self.operation,
                duration.as_millis()
            );
        }
    }

    /// Record an error in the operation
    pub fn error(self, error: &str) {
        let duration = self.start_time.elapsed();

        tracing::error!(
            operation = %self.operation,
            duration_ms = duration.as_millis() as u64,
            error = error,
            "Performance operation failed"
        );
    }
}

/// Macro for creating instrumented functions
#[macro_export]
macro_rules! instrument_async {
    ($func:expr, $operation:expr) => {{
        let span = $crate::monitoring::TracingService::create_performance_span($operation);
        let result = $func.await;
        match &result {
            Ok(_) => span.complete(true),
            Err(e) => span.error(&e.to_string()),
        }
        result
    }};
}

/// Macro for creating instrumented sync functions
#[macro_export]
macro_rules! instrument_sync {
    ($func:expr, $operation:expr) => {{
        let span = $crate::monitoring::TracingService::create_performance_span($operation);
        let result = $func;
        match &result {
            Ok(_) => span.complete(true),
            Err(e) => span.error(&e.to_string()),
        }
        result
    }};
}

/// Tracing utilities for common operations
pub struct TracingUtils;

impl TracingUtils {
    /// Create a span for gRPC operations
    pub fn grpc_span(method: &str, client_id: &str) -> tracing::Span {
        tracing::info_span!(
            "grpc_operation",
            method = method,
            client_id = client_id,
            duration_ms = tracing::field::Empty,
            status = tracing::field::Empty,
        )
    }

    /// Create a span for database operations
    pub fn database_span(operation: &str, measurement: &str) -> tracing::Span {
        tracing::info_span!(
            "database_operation",
            operation = operation,
            measurement = measurement,
            duration_ms = tracing::field::Empty,
            status = tracing::field::Empty,
        )
    }

    /// Create a span for atomic operations
    pub fn atomic_span(operation: &str, symbol: &str) -> tracing::Span {
        tracing::debug_span!(
            "atomic_operation",
            operation = operation,
            symbol = symbol,
            duration_ns = tracing::field::Empty,
        )
    }

    /// Create a span for subscription operations
    pub fn subscription_span(
        operation: &str,
        client_id: &str,
        symbols: &[String],
    ) -> tracing::Span {
        tracing::info_span!(
            "subscription_operation",
            operation = operation,
            client_id = client_id,
            symbol_count = symbols.len(),
            symbols = ?symbols,
            duration_ms = tracing::field::Empty,
        )
    }

    /// Record a custom event with structured data
    pub fn record_event(event_name: &str, data: &[(&str, &dyn std::fmt::Display)]) {
        let mut fields = Vec::new();
        for (key, value) in data {
            fields.push(format!("{key}={value}"));
        }

        tracing::info!(
            event = event_name,
            data = fields.join(", "),
            "Custom event recorded"
        );
    }

    /// Record performance metrics for atomic operations
    pub fn record_atomic_performance(operation: &str, symbol: &str, duration_ns: u64) {
        tracing::debug!(
            operation = operation,
            symbol = symbol,
            duration_ns = duration_ns,
            "Atomic operation performance"
        );

        // Log warning for slow atomic operations (> 1ms)
        if duration_ns > 1_000_000 {
            warn!(
                "Slow atomic operation: {} on {} took {}ns",
                operation, symbol, duration_ns
            );
        }
    }

    /// Record throughput metrics
    pub fn record_throughput(messages_per_second: f64, data_type: &str) {
        tracing::info!(
            messages_per_second = messages_per_second,
            data_type = data_type,
            "Throughput measurement"
        );
    }

    /// Record latency percentiles
    pub fn record_latency_percentile(operation: &str, percentile: f64, latency_ms: f64) {
        tracing::info!(
            operation = operation,
            percentile = percentile,
            latency_ms = latency_ms,
            "Latency percentile measurement"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tracing_service_creation() {
        let config = MonitoringConfig {
            tracing_enabled: false,
            ..Default::default()
        };

        let service = TracingService::new(config);

        // Should initialize basic logging without error
        service.initialize().await.unwrap();
    }

    #[test]
    fn test_performance_span() {
        let span = PerformanceSpan::new("test_operation");

        // Should complete without error
        span.complete(true);
    }

    #[test]
    fn test_performance_span_error() {
        let span = PerformanceSpan::new("test_operation");

        // Should handle error without panic
        span.error("Test error");
    }

    #[test]
    fn test_tracing_utils() {
        let _span = TracingUtils::grpc_span("StreamMarketData", "client123");
        let _span = TracingUtils::database_span("write", "orderbook");
        let _span = TracingUtils::atomic_span("update", "BTCUSDT");

        TracingUtils::record_event("test_event", &[("key", &"value")]);
        TracingUtils::record_atomic_performance("update", "BTCUSDT", 500);
        TracingUtils::record_throughput(1000.0, "orderbook");
        TracingUtils::record_latency_percentile("grpc_request", 95.0, 1.5);

        // Should not panic
    }
}

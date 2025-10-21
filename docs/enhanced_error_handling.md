# Enhanced Error Handling in Project Raven

This document describes the enhanced error handling system implemented in Project Raven, providing comprehensive error management, recovery strategies, and observability features.

## Overview

The enhanced error handling system provides:

- **Enhanced Error Context**: Rich metadata for error correlation and tracing
- **Recovery Strategies**: Automatic error recovery patterns
- **Error Metrics**: Monitoring and observability capabilities
- **Standardized Macros**: Consistent error handling patterns
- **Centralized Error Handler**: Unified error processing

## Key Components

### 1. Enhanced Error Context (`ErrorContextInfo`)

Provides rich metadata for error correlation and debugging:

```rust
pub struct ErrorContextInfo {
    pub correlation_id: Uuid,           // Unique identifier for tracing
    pub operation_id: Option<String>,   // Operation being performed
    pub user_id: Option<String>,        // User context
    pub request_id: Option<String>,     // Request identifier
    pub timestamp: SystemTime,          // When the error occurred
    pub service_name: Option<String>,   // Service name
    pub component: Option<String>,      // Component name
}
```

**Usage:**
```rust
let mut context = ErrorContextInfo::default();
context.operation_id = Some("database_write".to_string());
context.component = Some("influx_client".to_string());

let contextual_error = error.with_context(context);
contextual_error.log_with_context();
```

### 2. Recovery Strategies (`RecoveryStrategy`)

Defines automatic recovery patterns for different error types:

```rust
pub enum RecoveryStrategy {
    Retry { 
        max_attempts: u32, 
        delay_ms: u64,
        exponential_backoff: bool,
    },
    CircuitBreaker { 
        threshold: u32, 
        timeout_ms: u64,
        half_open_max_requests: u32,
    },
    DeadLetter { 
        max_retries: u32,
        retry_interval_ms: u64,
    },
    FailFast,
    Fallback { 
        fallback_value: String,
        alternative_operation: Option<String>,
    },
}
```

**Usage:**
```rust
let strategy = error.recovery_strategy();
match strategy {
    RecoveryStrategy::Retry { max_attempts, delay_ms, exponential_backoff } => {
        // Implement retry logic
    },
    RecoveryStrategy::FailFast => {
        // Fail immediately
    },
    // ... other strategies
}
```

### 3. Error Metrics (`ErrorMetrics`)

Provides monitoring and observability capabilities:

```rust
pub struct ErrorMetrics {
    pub error_counts: Arc<HashMap<String, AtomicU64>>,
    pub error_categories: Arc<HashMap<String, AtomicU64>>,
    pub recovery_attempts: Arc<HashMap<String, AtomicU64>>,
    pub recovery_successes: Arc<HashMap<String, AtomicU64>>,
    pub total_errors: AtomicU64,
    pub last_error_time: Arc<std::sync::Mutex<SystemTime>>,
}
```

**Usage:**
```rust
let metrics = Arc::new(ErrorMetrics::default());
metrics.track_error(&error);
let stats = metrics.get_stats();
```

### 4. Enhanced Error Context Traits

Provides operation-specific error context:

```rust
pub trait EnhancedErrorContext<T> {
    fn with_database_context(self, operation: &str, table: Option<&str>) -> RavenResult<T>;
    fn with_grpc_context(self, service: &str, method: &str) -> RavenResult<T>;
    fn with_subscription_context(self, client_id: &str, subscription_type: &str) -> RavenResult<T>;
    fn with_config_context(self, config_key: &str) -> RavenResult<T>;
    fn with_exchange_context(self, exchange: &str, symbol: Option<&str>) -> RavenResult<T>;
    fn with_operation_context(self, operation: &str, component: Option<&str>) -> RavenResult<T>;
}
```

**Usage:**
```rust
let result: Result<String, String> = Err("Connection failed".to_string());
let enhanced_result = result.with_database_context("write", Some("market_data"));
```

### 5. Standardized Error Handling Macros

Provides consistent error handling patterns:

```rust
// Database operations
handle_database_error!(result, "write", "trades");
handle_database_error!(result, "query");

// gRPC operations
handle_grpc_error!(result, "MarketDataService", "StreamMarketData");

// Subscription operations
handle_subscription_error!(result, "client_123", "orderbook");

// Configuration operations
handle_config_error!(result, "database.url");

// Exchange operations
handle_exchange_error!(result, "binance", "BTCUSDT");
handle_exchange_error!(result, "binance");

// General operations
handle_operation_error!(result, "data_processing", "citadel");
handle_operation_error!(result, "data_processing");
```

### 6. Centralized Error Handler (`ErrorHandler`)

Provides unified error processing:

```rust
pub struct ErrorHandler {
    pub metrics: Arc<ErrorMetrics>,
}

impl ErrorHandler {
    pub async fn handle_error<T>(
        &self,
        result: Result<T, RavenError>,
        context: Option<ErrorContextInfo>,
    ) -> Result<T, RavenError>;
    
    pub async fn handle_error_with_operation<T>(
        &self,
        result: Result<T, RavenError>,
        operation: &str,
        component: Option<&str>,
    ) -> Result<T, RavenError>;
}
```

**Usage:**
```rust
let error_handler = ErrorHandler::new();
let result = error_handler.handle_error_with_operation(
    operation_result, 
    "fetch_market_data", 
    Some("data_service")
).await;
```

## Error Categories and Recovery Strategies

### Database Errors
- **DatabaseConnection**: Retry with exponential backoff (3 attempts, 1000ms delay)
- **DatabaseWrite**: Retry with exponential backoff (2 attempts, 500ms delay)
- **DatabaseQuery**: Retry once (1 attempt, 1000ms delay)

### Network Errors
- **GrpcConnection**: Retry with exponential backoff (3 attempts, 2000ms delay)
- **StreamError**: Retry with exponential backoff (3 attempts, 2000ms delay)
- **ConnectionError**: Retry without backoff (2 attempts, 1000ms delay)

### Circuit Breaker Errors
- **CircuitBreakerOpen**: Use circuit breaker pattern (threshold: 5, timeout: 60s)

### External Service Errors
- **ExternalService**: Use circuit breaker pattern (threshold: 3, timeout: 30s)

### Data Processing Errors
- **DataValidation**: Fail fast (no retry)
- **DataSerialization**: Fail fast (no retry)
- **DataIngestion**: Fail fast (no retry)

### Configuration Errors
- **Configuration**: Fail fast (no retry)
- **InvalidConfigValue**: Fail fast (no retry)
- **ConfigError**: Fail fast (no retry)

### Dead Letter Queue Errors
- **DeadLetterQueueFull**: Send to DLQ (max retries: 3, interval: 5s)
- **DeadLetterProcessing**: Send to DLQ (max retries: 3, interval: 5s)

## Best Practices

### 1. Use Enhanced Context
Always provide rich context when creating errors:

```rust
let result = database_operation().await
    .with_database_context("write", Some("market_data"));
```

### 2. Leverage Recovery Strategies
Use the automatic recovery strategies:

```rust
let strategy = error.recovery_strategy();
match strategy {
    RecoveryStrategy::Retry { max_attempts, delay_ms, exponential_backoff } => {
        // Implement retry with the provided parameters
    },
    RecoveryStrategy::FailFast => {
        // Fail immediately without retry
    },
    _ => {
        // Handle other strategies
    }
}
```

### 3. Track Errors for Monitoring
Always track errors for observability:

```rust
let error_handler = ErrorHandler::new();
let result = error_handler.handle_error_with_operation(
    operation_result, 
    "operation_name", 
    Some("component_name")
).await;
```

### 4. Use Standardized Macros
Use the provided macros for consistent error handling:

```rust
// Instead of manual error wrapping
let result = operation().await
    .map_err(|e| RavenError::database_connection(e.to_string()));

// Use the macro
let result = handle_database_error!(operation().await, "write", "table_name");
```

### 5. Provide Rich Context
Include operation and component information:

```rust
let mut context = ErrorContextInfo::default();
context.operation_id = Some("fetch_market_data".to_string());
context.component = Some("data_service".to_string());
context.user_id = Some(user_id.to_string());

let contextual_error = error.with_context(context);
```

## Migration Guide

### From Old Error Handling

**Before:**
```rust
let result = database_operation().await
    .map_err(|e| RavenError::database_connection(e.to_string()))?;
```

**After:**
```rust
let result = handle_database_error!(database_operation().await, "write", "market_data")?;
```

### Adding Error Metrics

**Before:**
```rust
if let Err(e) = operation().await {
    tracing::error!("Operation failed: {}", e);
    return Err(e);
}
```

**After:**
```rust
let error_handler = ErrorHandler::new();
let result = error_handler.handle_error_with_operation(
    operation().await, 
    "operation_name", 
    Some("component_name")
).await?;
```

## Examples

See `examples/enhanced_error_handling.rs` for comprehensive examples of all the new error handling features.

## Performance Considerations

- Error metrics use atomic counters for thread-safe operations
- Context creation is lightweight with minimal allocation
- Recovery strategies are evaluated lazily
- Error tracking has minimal overhead

## Future Enhancements

- Integration with distributed tracing systems (Jaeger, Zipkin)
- Error rate limiting and circuit breaker integration
- Automatic error reporting to monitoring systems
- Error pattern detection and alerting

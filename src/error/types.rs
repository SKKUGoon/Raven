// Error Types and Definitions
// "The foundation of our error handling system"

use thiserror::Error;

/// Comprehensive error types for the market data subscription server
#[derive(Error, Debug, Clone)]
pub enum RavenError {
    // Database errors
    #[error("Database connection failed: {message}")]
    DatabaseConnection { message: String },

    #[error("Database write failed: {message}")]
    DatabaseWrite { message: String },

    #[error("Database query failed: {message}")]
    DatabaseQuery { message: String },

    #[error("Circuit breaker is open: {message}")]
    CircuitBreakerOpen { message: String },

    // gRPC and network errors
    #[error("gRPC connection failed: {message}")]
    GrpcConnection { message: String },

    #[error("Client disconnected: {client_id}")]
    ClientDisconnected { client_id: String },

    #[error("Maximum connections exceeded: {current}/{max}")]
    MaxConnectionsExceeded { current: usize, max: usize },

    #[error("Stream error: {message}")]
    StreamError { message: String },

    // Subscription management errors
    #[error("Subscription failed: {message}")]
    SubscriptionFailed { message: String },

    #[error("Client not found: {client_id}")]
    ClientNotFound { client_id: String },

    #[error("Invalid subscription request: {message}")]
    InvalidSubscription { message: String },

    // Data processing errors
    #[error("Data validation failed: {message}")]
    DataValidation { message: String },

    #[error("Data serialization failed: {message}")]
    DataSerialization { message: String },

    #[error("Data ingestion failed: {message}")]
    DataIngestion { message: String },

    // Configuration errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Invalid configuration value: {key} = {value}")]
    InvalidConfigValue { key: String, value: String },

    // System errors
    #[error("System resource exhausted: {resource}")]
    ResourceExhausted { resource: String },

    #[error("Timeout occurred: {operation} after {duration_ms}ms")]
    Timeout { operation: String, duration_ms: u64 },

    #[error("Authentication failed: {message}")]
    Authentication { message: String },

    #[error("Authorization failed: {message}")]
    Authorization { message: String },

    // Dead letter queue errors
    #[error("Dead letter queue full: {size}/{max_size}")]
    DeadLetterQueueFull { size: usize, max_size: usize },

    #[error("Dead letter processing failed: {message}")]
    DeadLetterProcessing { message: String },

    // Generic errors
    #[error("Internal server error: {message}")]
    Internal { message: String },

    #[error("External service error: {service}: {message}")]
    ExternalService { service: String, message: String },

    // Exchange errors
    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("Connection error: {message}")]
    ConnectionError { message: String },

    #[error("Config error: {message}")]
    ConfigError { message: String },

    #[error("Not implemented: {message}")]
    NotImplemented { message: String },

    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl RavenError {
    /// Create a database connection error
    pub fn database_connection<S: Into<String>>(message: S) -> Self {
        Self::DatabaseConnection {
            message: message.into(),
        }
    }

    /// Create a database write error
    pub fn database_write<S: Into<String>>(message: S) -> Self {
        Self::DatabaseWrite {
            message: message.into(),
        }
    }

    /// Create a database query error
    pub fn database_query<S: Into<String>>(message: S) -> Self {
        Self::DatabaseQuery {
            message: message.into(),
        }
    }

    /// Create a circuit breaker error
    pub fn circuit_breaker_open<S: Into<String>>(message: S) -> Self {
        Self::CircuitBreakerOpen {
            message: message.into(),
        }
    }

    /// Create a gRPC connection error
    pub fn grpc_connection<S: Into<String>>(message: S) -> Self {
        Self::GrpcConnection {
            message: message.into(),
        }
    }

    /// Create a client disconnected error
    pub fn client_disconnected<S: Into<String>>(client_id: S) -> Self {
        Self::ClientDisconnected {
            client_id: client_id.into(),
        }
    }

    /// Create a max connections exceeded error
    pub fn max_connections_exceeded(current: usize, max: usize) -> Self {
        Self::MaxConnectionsExceeded { current, max }
    }

    /// Create a stream error
    pub fn stream_error<S: Into<String>>(message: S) -> Self {
        Self::StreamError {
            message: message.into(),
        }
    }

    /// Create a subscription failed error
    pub fn subscription_failed<S: Into<String>>(message: S) -> Self {
        Self::SubscriptionFailed {
            message: message.into(),
        }
    }

    /// Create a client not found error
    pub fn client_not_found<S: Into<String>>(client_id: S) -> Self {
        Self::ClientNotFound {
            client_id: client_id.into(),
        }
    }

    /// Create an invalid subscription error
    pub fn invalid_subscription<S: Into<String>>(message: S) -> Self {
        Self::InvalidSubscription {
            message: message.into(),
        }
    }

    /// Create a data validation error
    pub fn data_validation<S: Into<String>>(message: S) -> Self {
        Self::DataValidation {
            message: message.into(),
        }
    }

    /// Create a data serialization error
    pub fn data_serialization<S: Into<String>>(message: S) -> Self {
        Self::DataSerialization {
            message: message.into(),
        }
    }

    /// Create a data ingestion error
    pub fn data_ingestion<S: Into<String>>(message: S) -> Self {
        Self::DataIngestion {
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create an invalid config value error
    pub fn invalid_config_value<K: Into<String>, V: Into<String>>(key: K, value: V) -> Self {
        Self::InvalidConfigValue {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Create a resource exhausted error
    pub fn resource_exhausted<S: Into<String>>(resource: S) -> Self {
        Self::ResourceExhausted {
            resource: resource.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(operation: S, duration_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            duration_ms,
        }
    }

    /// Create an authentication error
    pub fn authentication<S: Into<String>>(message: S) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create an authorization error
    pub fn authorization<S: Into<String>>(message: S) -> Self {
        Self::Authorization {
            message: message.into(),
        }
    }

    /// Create a dead letter queue full error
    pub fn dead_letter_queue_full(size: usize, max_size: usize) -> Self {
        Self::DeadLetterQueueFull { size, max_size }
    }

    /// Create a dead letter processing error
    pub fn dead_letter_processing<S: Into<String>>(message: S) -> Self {
        Self::DeadLetterProcessing {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal<S: Into<String>>(message: S) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Create an external service error
    pub fn external_service<S: Into<String>, M: Into<String>>(service: S, message: M) -> Self {
        Self::ExternalService {
            service: service.into(),
            message: message.into(),
        }
    }

    /// Create a parse error
    pub fn parse_error<S: Into<String>>(message: S) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Create a connection error
    pub fn connection_error<S: Into<String>>(message: S) -> Self {
        Self::ConnectionError {
            message: message.into(),
        }
    }

    /// Create a config error
    pub fn config_error<S: Into<String>>(message: S) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    /// Create a not implemented error
    pub fn not_implemented<S: Into<String>>(message: S) -> Self {
        Self::NotImplemented {
            message: message.into(),
        }
    }

    /// Create an internal error (new variant)
    pub fn internal_error<S: Into<String>>(message: S) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }
}

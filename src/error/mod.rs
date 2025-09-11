// Error Handling Module
// "Winter is coming - prepare for failures"

use std::fmt;
use thiserror::Error;
use tonic::Status;

/// Comprehensive error types for the market data subscription server
#[derive(Error, Debug)]
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

    /// Get error category for metrics and logging
    pub fn category(&self) -> &'static str {
        match self {
            Self::DatabaseConnection { .. }
            | Self::DatabaseWrite { .. }
            | Self::DatabaseQuery { .. } => "database",
            Self::CircuitBreakerOpen { .. } => "circuit_breaker",
            Self::GrpcConnection { .. }
            | Self::ClientDisconnected { .. }
            | Self::MaxConnectionsExceeded { .. }
            | Self::StreamError { .. } => "network",
            Self::SubscriptionFailed { .. }
            | Self::ClientNotFound { .. }
            | Self::InvalidSubscription { .. } => "subscription",
            Self::DataValidation { .. }
            | Self::DataSerialization { .. }
            | Self::DataIngestion { .. } => "data",
            Self::Configuration { .. } | Self::InvalidConfigValue { .. } => "configuration",
            Self::ResourceExhausted { .. } | Self::Timeout { .. } => "system",
            Self::Authentication { .. } | Self::Authorization { .. } => "security",
            Self::DeadLetterQueueFull { .. } | Self::DeadLetterProcessing { .. } => "dead_letter",
            Self::Internal { .. } | Self::ExternalService { .. } => "general",
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            // Retryable errors
            Self::DatabaseConnection { .. }
            | Self::DatabaseWrite { .. }
            | Self::GrpcConnection { .. }
            | Self::StreamError { .. }
            | Self::Timeout { .. }
            | Self::ExternalService { .. } => true,

            // Non-retryable errors
            Self::DatabaseQuery { .. }
            | Self::CircuitBreakerOpen { .. }
            | Self::ClientDisconnected { .. }
            | Self::MaxConnectionsExceeded { .. }
            | Self::SubscriptionFailed { .. }
            | Self::ClientNotFound { .. }
            | Self::InvalidSubscription { .. }
            | Self::DataValidation { .. }
            | Self::DataSerialization { .. }
            | Self::DataIngestion { .. }
            | Self::Configuration { .. }
            | Self::InvalidConfigValue { .. }
            | Self::ResourceExhausted { .. }
            | Self::Authentication { .. }
            | Self::Authorization { .. }
            | Self::DeadLetterQueueFull { .. }
            | Self::DeadLetterProcessing { .. }
            | Self::Internal { .. } => false,
        }
    }

    /// Get severity level for logging
    pub fn severity(&self) -> tracing::Level {
        match self {
            // Critical errors
            Self::DatabaseConnection { .. }
            | Self::CircuitBreakerOpen { .. }
            | Self::MaxConnectionsExceeded { .. }
            | Self::ResourceExhausted { .. }
            | Self::DeadLetterQueueFull { .. } => tracing::Level::ERROR,

            // Warning level errors
            Self::DatabaseWrite { .. }
            | Self::DatabaseQuery { .. }
            | Self::GrpcConnection { .. }
            | Self::StreamError { .. }
            | Self::Timeout { .. }
            | Self::DeadLetterProcessing { .. }
            | Self::ExternalService { .. } => tracing::Level::WARN,

            // Info level errors
            Self::ClientDisconnected { .. }
            | Self::SubscriptionFailed { .. }
            | Self::ClientNotFound { .. } => tracing::Level::INFO,

            // Debug level errors
            Self::InvalidSubscription { .. }
            | Self::DataValidation { .. }
            | Self::DataSerialization { .. }
            | Self::DataIngestion { .. }
            | Self::Configuration { .. }
            | Self::InvalidConfigValue { .. }
            | Self::Authentication { .. }
            | Self::Authorization { .. }
            | Self::Internal { .. } => tracing::Level::DEBUG,
        }
    }
}

/// Convert RavenError to tonic::Status for gRPC responses
impl From<RavenError> for Status {
    fn from(error: RavenError) -> Self {
        match error {
            RavenError::DatabaseConnection { message }
            | RavenError::DatabaseWrite { message }
            | RavenError::DatabaseQuery { message }
            | RavenError::CircuitBreakerOpen { message }
            | RavenError::ExternalService { message, .. } => {
                Status::unavailable(format!("Service unavailable: {message}"))
            }

            RavenError::GrpcConnection { message } | RavenError::StreamError { message } => {
                Status::aborted(format!("Connection error: {message}"))
            }

            RavenError::ClientDisconnected { client_id } => {
                Status::cancelled(format!("Client disconnected: {client_id}"))
            }

            RavenError::MaxConnectionsExceeded { current, max } => {
                Status::resource_exhausted(format!("Maximum connections exceeded: {current}/{max}"))
            }

            RavenError::SubscriptionFailed { message }
            | RavenError::InvalidSubscription { message } => {
                Status::invalid_argument(format!("Subscription error: {message}"))
            }

            RavenError::ClientNotFound { client_id } => {
                Status::not_found(format!("Client not found: {client_id}"))
            }

            RavenError::DataValidation { message }
            | RavenError::DataSerialization { message }
            | RavenError::DataIngestion { message } => {
                Status::invalid_argument(format!("Data error: {message}"))
            }

            RavenError::Configuration { message } => {
                Status::failed_precondition(format!("Configuration error: {message}"))
            }
            RavenError::InvalidConfigValue { key, value } => {
                Status::failed_precondition(format!("Invalid configuration: {key} = {value}"))
            }

            RavenError::ResourceExhausted { resource } => {
                Status::resource_exhausted(format!("Resource exhausted: {resource}"))
            }

            RavenError::Timeout {
                operation,
                duration_ms,
            } => Status::deadline_exceeded(format!(
                "Operation timed out: {operation} after {duration_ms}ms"
            )),

            RavenError::Authentication { message } => {
                Status::unauthenticated(format!("Authentication failed: {message}"))
            }

            RavenError::Authorization { message } => {
                Status::permission_denied(format!("Authorization failed: {message}"))
            }

            RavenError::DeadLetterQueueFull { size, max_size } => {
                Status::resource_exhausted(format!("Dead letter queue full: {size}/{max_size}"))
            }

            RavenError::DeadLetterProcessing { message } => {
                Status::internal(format!("Dead letter processing failed: {message}"))
            }

            RavenError::Internal { message } => {
                Status::internal(format!("Internal error: {message}"))
            }
        }
    }
}

/// Convert anyhow::Error to RavenError
impl From<anyhow::Error> for RavenError {
    fn from(error: anyhow::Error) -> Self {
        RavenError::internal(error.to_string())
    }
}

/// Convert std::io::Error to RavenError
impl From<std::io::Error> for RavenError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::TimedOut => {
                RavenError::timeout("IO operation", 0) // Duration not available from std::io::Error
            }
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset => RavenError::grpc_connection(error.to_string()),
            _ => RavenError::internal(error.to_string()),
        }
    }
}

/// Convert serde_json::Error to RavenError
impl From<serde_json::Error> for RavenError {
    fn from(error: serde_json::Error) -> Self {
        RavenError::data_serialization(error.to_string())
    }
}

/// Result type alias for convenience
pub type RavenResult<T> = Result<T, RavenError>;

/// Error context trait for adding context to errors
pub trait ErrorContext<T> {
    fn with_context<F>(self, f: F) -> RavenResult<T>
    where
        F: FnOnce() -> String;

    fn with_database_context(self) -> RavenResult<T>;
    fn with_grpc_context(self) -> RavenResult<T>;
    fn with_subscription_context(self) -> RavenResult<T>;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: fmt::Display,
{
    fn with_context<F>(self, f: F) -> RavenResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| RavenError::internal(format!("{}: {}", f(), e)))
    }

    fn with_database_context(self) -> RavenResult<T> {
        self.map_err(|e| RavenError::database_connection(e.to_string()))
    }

    fn with_grpc_context(self) -> RavenResult<T> {
        self.map_err(|e| RavenError::grpc_connection(e.to_string()))
    }

    fn with_subscription_context(self) -> RavenResult<T> {
        self.map_err(|e| RavenError::subscription_failed(e.to_string()))
    }
}

/// Macro for creating errors with context
#[macro_export]
macro_rules! raven_error {
    ($variant:ident, $($arg:expr),*) => {
        $crate::error::RavenError::$variant($($arg),*)
    };
}

/// Macro for early return with error logging
#[macro_export]
macro_rules! raven_bail {
    ($error:expr) => {
        {
            let error = $error;
            match error.severity() {
                tracing::Level::ERROR => tracing::error!(error = %error, "Operation failed"),
                tracing::Level::WARN => tracing::warn!(error = %error, "Operation failed"),
                tracing::Level::INFO => tracing::info!(error = %error, "Operation failed"),
                tracing::Level::DEBUG => tracing::debug!(error = %error, "Operation failed"),
                tracing::Level::TRACE => tracing::trace!(error = %error, "Operation failed"),
            }
            return Err(error);
        }
    };
}

/// Macro for error logging with context
#[macro_export]
macro_rules! log_error {
    ($error:expr, $($field:tt)*) => {
        {
            let error = $error;
            tracing::event!(
                error.severity(),
                error = %error,
                category = error.category(),
                retryable = error.is_retryable(),
                $($field)*
            );
        }
    };
}

#[cfg(test)]
mod tests;

// Error Classification and Analysis
// "Understanding the nature of our errors"

use crate::common::error::RavenError;

impl RavenError {
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
            Self::ParseError { .. }
            | Self::ConnectionError { .. }
            | Self::ConfigError { .. }
            | Self::NotImplemented { .. }
            | Self::InternalError { .. } => "exchange",
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
            | Self::Internal { .. }
            | Self::ParseError { .. }
            | Self::ConfigError { .. }
            | Self::NotImplemented { .. }
            | Self::InternalError { .. } => false,

            // Connection errors are retryable
            Self::ConnectionError { .. } => true,
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
            | Self::Internal { .. }
            | Self::ParseError { .. }
            | Self::ConfigError { .. }
            | Self::NotImplemented { .. }
            | Self::InternalError { .. } => tracing::Level::DEBUG,

            // Connection errors are warnings
            Self::ConnectionError { .. } => tracing::Level::WARN,
        }
    }
}

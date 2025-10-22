// Error Recovery Strategies
// "How to recover from different types of errors"

use crate::error::RavenError;

/// Error recovery strategies for different error types
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry the operation with exponential backoff
    Retry {
        max_attempts: u32,
        delay_ms: u64,
        exponential_backoff: bool,
    },
    /// Use circuit breaker pattern
    CircuitBreaker {
        threshold: u32,
        timeout_ms: u64,
        half_open_max_requests: u32,
    },
    /// Send to dead letter queue for later processing
    DeadLetter {
        max_retries: u32,
        retry_interval_ms: u64,
    },
    /// Fail immediately without retry
    FailFast,
    /// Use fallback value or alternative operation
    Fallback {
        fallback_value: String,
        alternative_operation: Option<String>,
    },
}

impl RavenError {
    /// Get recovery strategy for this error type
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            // Database errors - retry with exponential backoff
            Self::DatabaseConnection { .. } => RecoveryStrategy::Retry {
                max_attempts: 3,
                delay_ms: 1000,
                exponential_backoff: true,
            },
            Self::DatabaseWrite { .. } => RecoveryStrategy::Retry {
                max_attempts: 2,
                delay_ms: 500,
                exponential_backoff: true,
            },
            Self::DatabaseQuery { .. } => RecoveryStrategy::Retry {
                max_attempts: 1,
                delay_ms: 1000,
                exponential_backoff: false,
            },

            // Circuit breaker errors - use circuit breaker pattern
            Self::CircuitBreakerOpen { .. } => RecoveryStrategy::CircuitBreaker {
                threshold: 5,
                timeout_ms: 60000,
                half_open_max_requests: 3,
            },

            // Network errors - retry with backoff
            Self::GrpcConnection { .. } | Self::StreamError { .. } => RecoveryStrategy::Retry {
                max_attempts: 3,
                delay_ms: 2000,
                exponential_backoff: true,
            },
            Self::ConnectionError { .. } => RecoveryStrategy::Retry {
                max_attempts: 2,
                delay_ms: 1000,
                exponential_backoff: false,
            },

            // Timeout errors - retry once
            Self::Timeout { .. } => RecoveryStrategy::Retry {
                max_attempts: 1,
                delay_ms: 1000,
                exponential_backoff: false,
            },

            // External service errors - retry with circuit breaker
            Self::ExternalService { .. } => RecoveryStrategy::CircuitBreaker {
                threshold: 3,
                timeout_ms: 30000,
                half_open_max_requests: 2,
            },

            // Dead letter queue errors - send to DLQ
            Self::DeadLetterQueueFull { .. } | Self::DeadLetterProcessing { .. } => {
                RecoveryStrategy::DeadLetter {
                    max_retries: 3,
                    retry_interval_ms: 5000,
                }
            }

            // Data processing errors - fail fast (usually indicate data issues)
            Self::DataValidation { .. }
            | Self::DataSerialization { .. }
            | Self::DataIngestion { .. } => RecoveryStrategy::FailFast,

            // Configuration errors - fail fast (usually indicate setup issues)
            Self::Configuration { .. }
            | Self::InvalidConfigValue { .. }
            | Self::ConfigError { .. } => RecoveryStrategy::FailFast,

            // Authentication/Authorization - fail fast
            Self::Authentication { .. } | Self::Authorization { .. } => RecoveryStrategy::FailFast,

            // Client errors - fail fast (usually indicate client issues)
            Self::ClientDisconnected { .. }
            | Self::ClientNotFound { .. }
            | Self::InvalidSubscription { .. } => RecoveryStrategy::FailFast,

            // Resource exhaustion - fail fast
            Self::ResourceExhausted { .. } | Self::MaxConnectionsExceeded { .. } => {
                RecoveryStrategy::FailFast
            }

            // Parse errors - fail fast
            Self::ParseError { .. } => RecoveryStrategy::FailFast,

            // Internal errors - fail fast
            Self::Internal { .. } | Self::InternalError { .. } | Self::NotImplemented { .. } => {
                RecoveryStrategy::FailFast
            }

            // Subscription errors - retry once
            Self::SubscriptionFailed { .. } => RecoveryStrategy::Retry {
                max_attempts: 1,
                delay_ms: 2000,
                exponential_backoff: false,
            },
        }
    }
}

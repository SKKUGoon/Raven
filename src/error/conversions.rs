// Error Conversions and Integrations
// "Converting between different error types and external systems"

use crate::error::RavenError;
use tonic::Status;

/// Convert RavenError to tonic::Status for gRPC responses
impl From<RavenError> for Status {
    fn from(error: RavenError) -> Self {
        match &error {
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

            RavenError::ParseError { message } => {
                Status::invalid_argument(format!("Parse error: {message}"))
            }

            RavenError::ConnectionError { message } => {
                Status::unavailable(format!("Connection error: {message}"))
            }

            RavenError::ConfigError { message } => {
                Status::failed_precondition(format!("Config error: {message}"))
            }

            RavenError::NotImplemented { message } => {
                Status::unimplemented(format!("Not implemented: {message}"))
            }

            RavenError::InternalError { message } => {
                Status::internal(format!("Internal error: {message}"))
            }
        }
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

/// Convert prometheus::Error to RavenError
impl From<prometheus::Error> for RavenError {
    fn from(error: prometheus::Error) -> Self {
        RavenError::internal(format!("Metrics error: {error}"))
    }
}

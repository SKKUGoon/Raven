// Error Context Traits
// "Enhanced error context with operation-specific information"

use crate::error::RavenResult;
use std::fmt;

/// Error context trait for adding context to errors
pub trait ErrorContext<T> {
    fn with_context<F>(self, f: F) -> RavenResult<T>
    where
        F: FnOnce() -> String;

    fn with_database_context(self) -> RavenResult<T>;
    fn with_grpc_context(self) -> RavenResult<T>;
    fn with_subscription_context(self) -> RavenResult<T>;
}

/// Enhanced error context trait with more specific information
pub trait EnhancedErrorContext<T> {
    fn with_database_context(self, operation: &str, table: Option<&str>) -> RavenResult<T>;
    fn with_grpc_context(self, service: &str, method: &str) -> RavenResult<T>;
    fn with_subscription_context(self, client_id: &str, subscription_type: &str) -> RavenResult<T>;
    fn with_config_context(self, config_key: &str) -> RavenResult<T>;
    fn with_exchange_context(self, exchange: &str, symbol: Option<&str>) -> RavenResult<T>;
    fn with_operation_context(self, operation: &str, component: Option<&str>) -> RavenResult<T>;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: fmt::Display,
{
    fn with_context<F>(self, f: F) -> RavenResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| crate::raven_error!(internal, format!("{}: {}", f(), e)))
    }

    fn with_database_context(self) -> RavenResult<T> {
        self.map_err(|e| crate::raven_error!(database_connection, e.to_string()))
    }

    fn with_grpc_context(self) -> RavenResult<T> {
        self.map_err(|e| crate::raven_error!(grpc_connection, e.to_string()))
    }

    fn with_subscription_context(self) -> RavenResult<T> {
        self.map_err(|e| crate::raven_error!(subscription_failed, e.to_string()))
    }
}

impl<T, E> EnhancedErrorContext<T> for Result<T, E>
where
    E: fmt::Display,
{
    fn with_database_context(self, operation: &str, table: Option<&str>) -> RavenResult<T> {
        let context = match table {
            Some(table) => format!("Database {operation} operation on table {table}"),
            None => format!("Database {operation} operation"),
        };
        self.map_err(|e| crate::raven_error!(database_connection, format!("{context}: {e}")))
    }

    fn with_grpc_context(self, service: &str, method: &str) -> RavenResult<T> {
        let context = format!("gRPC {service} service {method} method");
        self.map_err(|e| crate::raven_error!(grpc_connection, format!("{context}: {e}")))
    }

    fn with_subscription_context(self, client_id: &str, subscription_type: &str) -> RavenResult<T> {
        let context = format!("Subscription {subscription_type} for client {client_id}",);
        self.map_err(|e| crate::raven_error!(subscription_failed, format!("{context}: {e}")))
    }

    fn with_config_context(self, config_key: &str) -> RavenResult<T> {
        let context = format!("Configuration key: {config_key}");
        self.map_err(|e| crate::raven_error!(configuration, format!("{context}: {e}")))
    }

    fn with_exchange_context(self, exchange: &str, symbol: Option<&str>) -> RavenResult<T> {
        let context = match symbol {
            Some(symbol) => format!("Exchange {exchange} symbol {symbol}"),
            None => format!("Exchange {exchange}"),
        };
        self.map_err(|e| crate::raven_error!(connection_error, format!("{context}: {e}")))
    }

    fn with_operation_context(self, operation: &str, component: Option<&str>) -> RavenResult<T> {
        let context = match component {
            Some(component) => format!("{operation} operation in {component}"),
            None => format!("{operation} operation"),
        };
        self.map_err(|e| crate::raven_error!(internal, format!("{context}: {e}")))
    }
}

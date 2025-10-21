// Error Context and Tracing
// "Rich metadata for error correlation and debugging"

use crate::error::RavenError;
use std::time::SystemTime;
use uuid::Uuid;

/// Error context for correlation and tracing
#[derive(Debug, Clone)]
pub struct ErrorContextInfo {
    pub correlation_id: Uuid,
    pub operation_id: Option<String>,
    pub user_id: Option<String>,
    pub request_id: Option<String>,
    pub timestamp: SystemTime,
    pub service_name: Option<String>,
    pub component: Option<String>,
}

impl Default for ErrorContextInfo {
    fn default() -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            operation_id: None,
            user_id: None,
            request_id: None,
            timestamp: SystemTime::now(),
            service_name: None,
            component: None,
        }
    }
}

/// Contextual error with additional metadata
#[derive(Debug)]
pub struct ContextualRavenError {
    pub error: RavenError,
    pub context: ErrorContextInfo,
}

impl RavenError {
    /// Create a contextual error with additional metadata
    pub fn with_context(self, context: ErrorContextInfo) -> ContextualRavenError {
        ContextualRavenError {
            error: self,
            context,
        }
    }

    /// Create a contextual error with default context
    pub fn with_default_context(self) -> ContextualRavenError {
        self.with_context(ErrorContextInfo::default())
    }
}

impl ContextualRavenError {
    /// Log the error with full context information
    pub fn log_with_context(&self) {
        tracing::error!(
            error = %self.error,
            correlation_id = %self.context.correlation_id,
            operation_id = ?self.context.operation_id,
            user_id = ?self.context.user_id,
            request_id = ?self.context.request_id,
            service_name = ?self.context.service_name,
            component = ?self.context.component,
            category = self.error.category(),
            retryable = self.error.is_retryable(),
            "Operation failed with context"
        );
    }

    /// Get the underlying error
    pub fn error(&self) -> &RavenError {
        &self.error
    }

    /// Get the error context
    pub fn context(&self) -> &ErrorContextInfo {
        &self.context
    }
}

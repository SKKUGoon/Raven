// Error Handler and Centralized Processing
// "Unified error processing and management"

use crate::common::error::{ErrorContextInfo, ErrorMetrics, RavenError};
use std::sync::Arc;

/// Centralized error handler for consistent error processing
pub struct ErrorHandler {
    pub metrics: Arc<ErrorMetrics>,
}

impl ErrorHandler {
    /// Create a new error handler
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(ErrorMetrics::default()),
        }
    }

    /// Handle an error with automatic recovery strategy application
    pub async fn handle_error<T>(
        &self,
        result: Result<T, RavenError>,
        context: Option<ErrorContextInfo>,
    ) -> Result<T, RavenError> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                // Track the error
                self.metrics.track_error(&error);

                // Create contextual error if context provided
                if let Some(ctx) = context {
                    let contextual_error = error.with_context(ctx);
                    contextual_error.log_with_context();
                    Err(contextual_error.error)
                } else {
                    // Log without context
                    let error_clone = error.clone();
                    tracing::error!(
                        error = %error_clone,
                        category = error_clone.category(),
                        retryable = error_clone.is_retryable(),
                        "Operation failed"
                    );
                    Err(error)
                }
            }
        }
    }

    /// Handle an error with automatic context generation
    pub async fn handle_error_with_operation<T>(
        &self,
        result: Result<T, RavenError>,
        operation: &str,
        component: Option<&str>,
    ) -> Result<T, RavenError> {
        let context = ErrorContextInfo {
            operation_id: Some(operation.to_string()),
            component: component.map(|c| c.to_string()),
            ..Default::default()
        };

        self.handle_error(result, Some(context)).await
    }

    /// Get error metrics
    pub fn get_metrics(&self) -> Arc<ErrorMetrics> {
        Arc::clone(&self.metrics)
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

// Error Metrics and Monitoring
// "Observability and performance tracking for errors"

use crate::error::RavenError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

/// Error metrics for monitoring and observability
#[derive(Debug)]
pub struct ErrorMetrics {
    pub error_counts: Arc<HashMap<String, AtomicU64>>,
    pub error_categories: Arc<HashMap<String, AtomicU64>>,
    pub recovery_attempts: Arc<HashMap<String, AtomicU64>>,
    pub recovery_successes: Arc<HashMap<String, AtomicU64>>,
    pub total_errors: AtomicU64,
    pub last_error_time: Arc<std::sync::Mutex<SystemTime>>,
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self {
            error_counts: Arc::new(HashMap::new()),
            error_categories: Arc::new(HashMap::new()),
            recovery_attempts: Arc::new(HashMap::new()),
            recovery_successes: Arc::new(HashMap::new()),
            total_errors: AtomicU64::new(0),
            last_error_time: Arc::new(std::sync::Mutex::new(SystemTime::now())),
        }
    }
}

impl ErrorMetrics {
    /// Track an error occurrence
    pub fn track_error(&self, error: &RavenError) {
        let error_type = format!("{error:?}");
        let category = error.category().to_string();

        // Increment total error count
        self.total_errors.fetch_add(1, Ordering::Relaxed);

        // Update last error time
        if let Ok(mut last_time) = self.last_error_time.lock() {
            *last_time = SystemTime::now();
        }

        // Track error counts (simplified for now - in production you'd want proper concurrent access)
        tracing::debug!(
            error_type = %error_type,
            category = %category,
            "Error tracked"
        );
    }

    /// Track a recovery attempt
    pub fn track_recovery_attempt(&self, error_type: &str, success: bool) {
        tracing::debug!(
            error_type = %error_type,
            success = success,
            "Recovery attempt tracked"
        );
    }

    /// Get error statistics
    pub fn get_stats(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();
        stats.insert(
            "total_errors".to_string(),
            self.total_errors.load(Ordering::Relaxed),
        );
        stats
    }
}

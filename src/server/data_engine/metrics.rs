use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;

/// Data Engine metrics for monitoring validation performance
#[derive(Debug)]
pub struct DataEngineMetrics {
    pub total_ingested: AtomicU64,
    pub total_validated: AtomicU64,
    pub total_written: AtomicU64,
    pub total_failed: AtomicU64,
    pub validation_errors: AtomicU64,
    pub sanitization_fixes: AtomicU64,
    pub dead_letter_entries: AtomicU64,
}

impl Default for DataEngineMetrics {
    fn default() -> Self {
        Self {
            total_ingested: AtomicU64::new(0),
            total_validated: AtomicU64::new(0),
            total_written: AtomicU64::new(0),
            total_failed: AtomicU64::new(0),
            validation_errors: AtomicU64::new(0),
            sanitization_fixes: AtomicU64::new(0),
            dead_letter_entries: AtomicU64::new(0),
        }
    }
}

impl DataEngineMetrics {
    /// Get metrics as map
    pub fn to_map(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();
        metrics.insert(
            "total_ingested".to_string(),
            self.total_ingested.load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_validated".to_string(),
            self.total_validated.load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_written".to_string(),
            self.total_written.load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_failed".to_string(),
            self.total_failed.load(Ordering::Relaxed),
        );
        metrics.insert(
            "validation_errors".to_string(),
            self.validation_errors.load(Ordering::Relaxed),
        );
        metrics.insert(
            "sanitization_fixes".to_string(),
            self.sanitization_fixes.load(Ordering::Relaxed),
        );
        metrics.insert(
            "dead_letter_entries".to_string(),
            self.dead_letter_entries.load(Ordering::Relaxed),
        );
        metrics
    }
}


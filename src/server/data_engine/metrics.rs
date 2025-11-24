use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;

/// Data Engine metrics for monitoring performance
#[derive(Debug)]
pub struct DataEngineMetrics {
    pub total_ingested: AtomicU64,
    pub total_written: AtomicU64,
    pub total_failed: AtomicU64,
}

impl Default for DataEngineMetrics {
    fn default() -> Self {
        Self {
            total_ingested: AtomicU64::new(0),
            total_written: AtomicU64::new(0),
            total_failed: AtomicU64::new(0),
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
            "total_written".to_string(),
            self.total_written.load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_failed".to_string(),
            self.total_failed.load(Ordering::Relaxed),
        );
        metrics
    }
}


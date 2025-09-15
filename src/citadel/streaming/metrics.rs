// Performance metrics for the snapshot service

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Performance metrics for the snapshot service
#[derive(Debug, Default)]
pub struct SnapshotMetrics {
    /// Total number of snapshots captured
    pub total_snapshots: AtomicU64,
    /// Total number of orderbook snapshots
    pub orderbook_snapshots: AtomicU64,
    /// Total number of trade snapshots
    pub trade_snapshots: AtomicU64,
    /// Total number of snapshots written to database
    pub database_writes: AtomicU64,
    /// Total number of snapshots broadcast to clients
    pub client_broadcasts: AtomicU64,
    /// Total number of failed operations
    pub failed_operations: AtomicU64,
    /// Last snapshot timestamp
    pub last_snapshot_time: AtomicU64,
    /// Average snapshot capture time in nanoseconds
    pub avg_capture_time_ns: AtomicU64,
    /// Average database write time in nanoseconds
    pub avg_write_time_ns: AtomicU64,
}

impl SnapshotMetrics {
    /// Get metrics as a HashMap for monitoring
    pub fn to_map(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();
        metrics.insert(
            "total_snapshots".to_string(),
            self.total_snapshots.load(Ordering::Relaxed),
        );
        metrics.insert(
            "orderbook_snapshots".to_string(),
            self.orderbook_snapshots.load(Ordering::Relaxed),
        );
        metrics.insert(
            "trade_snapshots".to_string(),
            self.trade_snapshots.load(Ordering::Relaxed),
        );
        metrics.insert(
            "database_writes".to_string(),
            self.database_writes.load(Ordering::Relaxed),
        );
        metrics.insert(
            "client_broadcasts".to_string(),
            self.client_broadcasts.load(Ordering::Relaxed),
        );
        metrics.insert(
            "failed_operations".to_string(),
            self.failed_operations.load(Ordering::Relaxed),
        );
        metrics.insert(
            "last_snapshot_time".to_string(),
            self.last_snapshot_time.load(Ordering::Relaxed),
        );
        metrics.insert(
            "avg_capture_time_ns".to_string(),
            self.avg_capture_time_ns.load(Ordering::Relaxed),
        );
        metrics.insert(
            "avg_write_time_ns".to_string(),
            self.avg_write_time_ns.load(Ordering::Relaxed),
        );
        metrics
    }

    /// Update average capture time
    pub fn update_capture_time(&self, duration: Duration) {
        let new_time = duration.as_nanos() as u64;
        let current_avg = self.avg_capture_time_ns.load(Ordering::Relaxed);
        // Simple moving average approximation
        let new_avg = if current_avg == 0 {
            new_time
        } else {
            (current_avg * 9 + new_time) / 10
        };
        self.avg_capture_time_ns.store(new_avg, Ordering::Relaxed);
    }

    /// Update average write time
    pub fn update_write_time(&self, duration: Duration) {
        let new_time = duration.as_nanos() as u64;
        let current_avg = self.avg_write_time_ns.load(Ordering::Relaxed);
        // Simple moving average approximation
        let new_avg = if current_avg == 0 {
            new_time
        } else {
            (current_avg * 9 + new_time) / 10
        };
        self.avg_write_time_ns.store(new_avg, Ordering::Relaxed);
    }
}

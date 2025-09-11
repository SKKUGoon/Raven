// Configuration for the snapshot service

use std::time::Duration;

/// Configuration for the snapshot service
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Interval between snapshots (default: 5ms)
    pub snapshot_interval: Duration,
    /// Maximum number of snapshots to batch before writing to database
    pub max_batch_size: usize,
    /// Timeout for database writes
    pub write_timeout: Duration,
    /// Whether to broadcast snapshots to gRPC clients
    pub broadcast_enabled: bool,
    /// Whether to persist snapshots to database
    pub persistence_enabled: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_interval: Duration::from_millis(5),
            max_batch_size: 1000,
            write_timeout: Duration::from_millis(100),
            broadcast_enabled: true,
            persistence_enabled: true,
        }
    }
}

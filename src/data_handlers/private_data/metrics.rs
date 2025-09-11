use std::sync::atomic::AtomicU64;

/// Performance metrics
#[derive(Debug, Default)]
pub struct PrivateDataMetrics {
    pub wallet_updates_processed: AtomicU64,
    pub position_updates_processed: AtomicU64,
    pub total_failed: AtomicU64,
    pub access_denied_count: AtomicU64,
}

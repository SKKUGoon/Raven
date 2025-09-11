use std::sync::atomic::AtomicU64;

/// Performance metrics for monitoring
#[derive(Debug)]
pub struct LowFrequencyMetrics {
    pub candles_processed: AtomicU64,
    pub funding_rates_processed: AtomicU64,
    pub total_failed: AtomicU64,
    pub channel_queue_size: AtomicU64,
    pub last_processed_time: AtomicU64,
}

impl Default for LowFrequencyMetrics {
    fn default() -> Self {
        Self {
            candles_processed: AtomicU64::new(0),
            funding_rates_processed: AtomicU64::new(0),
            total_failed: AtomicU64::new(0),
            channel_queue_size: AtomicU64::new(0),
            last_processed_time: AtomicU64::new(0),
        }
    }
}

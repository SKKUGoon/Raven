use std::time::Duration;

/// Configuration for low-frequency handler
#[derive(Debug, Clone)]
pub struct LowFrequencyConfig {
    pub channel_buffer_size: usize,
    pub max_retry_attempts: u32,
    pub retry_delay: Duration,
    pub max_items_per_symbol: usize,
    pub processing_batch_size: usize,
    pub processing_interval: Duration,
}

impl Default for LowFrequencyConfig {
    fn default() -> Self {
        Self {
            channel_buffer_size: 10000,
            max_retry_attempts: 3,
            retry_delay: Duration::from_secs(5),
            max_items_per_symbol: 1000,
            processing_batch_size: 100,
            processing_interval: Duration::from_millis(100),
        }
    }
}

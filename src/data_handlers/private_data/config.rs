use std::time::Duration;

/// Configuration
#[derive(Debug, Clone)]
pub struct PrivateDataConfig {
    pub max_items_per_user: usize,
    pub processing_interval: Duration,
    pub encryption_enabled: bool,
}

impl Default for PrivateDataConfig {
    fn default() -> Self {
        Self {
            max_items_per_user: 500,
            processing_interval: Duration::from_millis(200),
            encryption_enabled: true,
        }
    }
}

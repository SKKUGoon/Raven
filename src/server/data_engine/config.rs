use serde::{Deserialize, Serialize};

/// Configuration for the Data Engine validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataEngineConfig {
    /// Enable strict validation mode
    pub strict_validation: bool,
    /// Maximum allowed price deviation (percentage)
    pub max_price_deviation: f64,
    /// Maximum allowed quantity
    pub max_quantity: f64,
    /// Minimum allowed price
    pub min_price: f64,
    /// Maximum allowed price
    pub max_price: f64,
    /// Enable data sanitization
    pub enable_sanitization: bool,
    /// Maximum age of data in seconds
    pub max_data_age_seconds: u64,
    /// Enable dead letter queue
    pub enable_dead_letter_queue: bool,
}

impl Default for DataEngineConfig {
    fn default() -> Self {
        Self {
            strict_validation: true,
            max_price_deviation: 10.0, // 10%
            max_quantity: 1000000.0,
            min_price: 0.00000001,
            max_price: 1000000.0,
            enable_sanitization: true,
            max_data_age_seconds: 300, // 5 minutes
            enable_dead_letter_queue: true,
        }
    }
}


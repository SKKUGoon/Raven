use serde::{Deserialize, Serialize};

use super::config::DataEngineConfig;

/// Validation rules for market data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    pub min_price: f64,
    pub max_price: f64,
    pub min_quantity: f64,
    pub max_quantity: f64,
    pub max_spread_percentage: f64,
    pub max_price_deviation: f64,
    pub required_fields: Vec<String>,
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self {
            min_price: 0.00000001,
            max_price: 1000000.0,
            min_quantity: 0.00000001,
            max_quantity: 1000000.0,
            max_spread_percentage: 5.0,
            max_price_deviation: 10.0,
            required_fields: vec![
                "symbol".to_string(),
                "timestamp".to_string(),
                "exchange".to_string(),
            ],
        }
    }
}

impl From<&DataEngineConfig> for ValidationRules {
    fn from(config: &DataEngineConfig) -> Self {
        Self {
            min_price: config.min_price,
            max_price: config.max_price,
            min_quantity: 0.00000001, // Config doesn't specify min quantity, keeping default
            max_quantity: config.max_quantity,
            max_spread_percentage: 5.0, // Config doesn't specify max spread, keeping default
            max_price_deviation: config.max_price_deviation,
            required_fields: vec![
                "symbol".to_string(),
                "timestamp".to_string(),
                "exchange".to_string(),
            ],
        }
    }
}


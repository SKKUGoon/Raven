use serde::{Deserialize, Serialize};

use crate::common::config::validation::ConfigSection;
use crate::common::error::RavenResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DataProcessingConfig {
    pub high_frequency_buffer_size: usize,
    pub low_frequency_buffer_size: usize,
    pub private_data_buffer_size: usize,
    pub data_validation_enabled: bool,
    pub sanitization_enabled: bool,
    pub dead_letter_queue_size: usize,
}

impl Default for DataProcessingConfig {
    fn default() -> Self {
        Self {
            high_frequency_buffer_size: 10_000,
            low_frequency_buffer_size: 1_000,
            private_data_buffer_size: 500,
            data_validation_enabled: true,
            sanitization_enabled: true,
            dead_letter_queue_size: 1_000,
        }
    }
}

impl ConfigSection for DataProcessingConfig {
    const KEY: &'static str = "data_processing";

    fn validate(&self) -> RavenResult<()> {
        Ok(())
    }
}

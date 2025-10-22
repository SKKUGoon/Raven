use serde::{Deserialize, Serialize};

use crate::config::validation::ConfigSection;
use crate::error::RavenResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BatchingConfig {
    pub database_writes: BatchConfig,
    pub client_broadcasts: BatchConfig,
    pub snapshot_captures: BatchConfig,
    pub metrics_collection: BatchConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BatchConfig {
    pub size: usize,
    pub timeout_ms: u64,
    pub max_memory_mb: usize,
    pub compression_threshold: usize,
}

impl Default for BatchingConfig {
    fn default() -> Self {
        Self {
            database_writes: BatchConfig {
                size: 1_000,
                timeout_ms: 5,
                max_memory_mb: 100,
                compression_threshold: 500,
            },
            client_broadcasts: BatchConfig {
                size: 100,
                timeout_ms: 1,
                max_memory_mb: 50,
                compression_threshold: 50,
            },
            snapshot_captures: BatchConfig {
                size: 50,
                timeout_ms: 5,
                max_memory_mb: 25,
                compression_threshold: 25,
            },
            metrics_collection: BatchConfig {
                size: 500,
                timeout_ms: 1_000,
                max_memory_mb: 10,
                compression_threshold: 250,
            },
        }
    }
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            size: 1_000,
            timeout_ms: 5,
            max_memory_mb: 100,
            compression_threshold: 500,
        }
    }
}

impl BatchConfig {
    pub fn validate(&self, name: &str) -> RavenResult<()> {
        if self.size == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                format!("{name}.size"),
                self.size.to_string(),
            ));
        }

        if self.timeout_ms == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                format!("{name}.timeout_ms"),
                self.timeout_ms.to_string(),
            ));
        }

        if self.max_memory_mb == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                format!("{name}.max_memory_mb"),
                self.max_memory_mb.to_string(),
            ));
        }

        Ok(())
    }
}

impl ConfigSection for BatchingConfig {
    const KEY: &'static str = "batching";

    fn validate(&self) -> RavenResult<()> {
        self.database_writes.validate("batching.database_writes")?;
        self.client_broadcasts
            .validate("batching.client_broadcasts")?;
        self.snapshot_captures
            .validate("batching.snapshot_captures")?;
        self.metrics_collection
            .validate("batching.metrics_collection")?;
        Ok(())
    }
}

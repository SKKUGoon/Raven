use serde::{Deserialize, Serialize};

use crate::common::config::validation::ConfigSection;
use crate::common::error::RavenResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub influx_url: String,
    pub bucket: String,
    pub org: String,
    pub token: Option<String>,
    pub connection_pool_size: usize,
    pub connection_timeout_seconds: u64,
    pub write_timeout_seconds: u64,
    pub query_timeout_seconds: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_seconds: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            influx_url: "http://localhost:8086".to_string(),
            bucket: "crypto".to_string(),
            org: "raven".to_string(),
            token: None,
            connection_pool_size: 20,
            connection_timeout_seconds: 10,
            write_timeout_seconds: 5,
            query_timeout_seconds: 30,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout_seconds: 60,
        }
    }
}

impl ConfigSection for DatabaseConfig {
    const KEY: &'static str = "database";

    fn validate(&self) -> RavenResult<()> {
        if self.influx_url.is_empty() {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "database.influx_url",
                "<empty>".to_string(),
            ));
        }

        if self.bucket.is_empty() {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "database.bucket",
                "<empty>".to_string(),
            ));
        }

        if self.org.is_empty() {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "database.org",
                "<empty>".to_string(),
            ));
        }

        if self.connection_pool_size == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "database.connection_pool_size",
                self.connection_pool_size.to_string(),
            ));
        }

        Ok(())
    }
}

use serde::{Deserialize, Serialize};

use crate::common::config::validation::ConfigSection;
use crate::common::error::RavenResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub heartbeat_interval_seconds: u64,
    pub client_timeout_seconds: u64,
    pub enable_compression: bool,
    pub max_message_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 50051,
            max_connections: 1000,
            heartbeat_interval_seconds: 30,
            client_timeout_seconds: 60,
            enable_compression: true,
            max_message_size: 4 * 1024 * 1024, // 4MB
        }
    }
}

impl ServerConfig {
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl ConfigSection for ServerConfig {
    const KEY: &'static str = "server";

    fn validate(&self) -> RavenResult<()> {
        if self.port == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "server.port",
                self.port.to_string(),
            ));
        }

        if self.max_connections == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "server.max_connections",
                self.max_connections.to_string(),
            ));
        }

        if self.max_message_size == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "server.max_message_size",
                self.max_message_size.to_string(),
            ));
        }

        Ok(())
    }
}

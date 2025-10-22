use serde::{Deserialize, Serialize};

use crate::config::validation::ConfigSection;
use crate::error::RavenResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitoringConfig {
    pub metrics_enabled: bool,
    pub metrics_port: u16,
    pub health_check_port: u16,
    pub tracing_enabled: bool,
    pub log_level: String,
    pub performance_monitoring: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            metrics_port: 9090,
            health_check_port: 8080,
            tracing_enabled: true,
            log_level: "info".to_string(),
            performance_monitoring: true,
        }
    }
}

impl ConfigSection for MonitoringConfig {
    const KEY: &'static str = "monitoring";

    fn validate(&self) -> RavenResult<()> {
        const ALLOWED_LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

        if !ALLOWED_LEVELS.contains(&self.log_level.as_str()) {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "monitoring.log_level",
                self.log_level.clone(),
            ));
        }

        if self.metrics_port == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "monitoring.metrics_port",
                self.metrics_port.to_string(),
            ));
        }

        if self.health_check_port == 0 {
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                "monitoring.health_check_port",
                self.health_check_port.to_string(),
            ));
        }

        Ok(())
    }
}

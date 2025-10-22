use serde::{Deserialize, Serialize};

use crate::config::validation::ConfigSection;
use crate::error::{RavenError, RavenResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionPolicies {
    pub high_frequency: RetentionPolicy,
    pub low_frequency: RetentionPolicy,
    pub private_data: RetentionPolicy,
    pub system_logs: RetentionPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionPolicy {
    pub full_resolution_days: u32,
    pub downsampled_days: u32,
    pub archive_days: u32,
    pub auto_cleanup: bool,
    pub compression_enabled: bool,
}

impl Default for RetentionPolicies {
    fn default() -> Self {
        Self {
            high_frequency: RetentionPolicy {
                full_resolution_days: 7,
                downsampled_days: 30,
                archive_days: 90,
                auto_cleanup: true,
                compression_enabled: true,
            },
            low_frequency: RetentionPolicy {
                full_resolution_days: 730, // 2 years
                downsampled_days: 1_095,   // 3 years
                archive_days: 1_825,       // 5 years
                auto_cleanup: true,
                compression_enabled: true,
            },
            private_data: RetentionPolicy {
                full_resolution_days: 365, // 1 year
                downsampled_days: 730,     // 2 years
                archive_days: 1_095,       // 3 years
                auto_cleanup: false,
                compression_enabled: true,
            },
            system_logs: RetentionPolicy {
                full_resolution_days: 30,
                downsampled_days: 90,
                archive_days: 180,
                auto_cleanup: true,
                compression_enabled: true,
            },
        }
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            full_resolution_days: 7,
            downsampled_days: 30,
            archive_days: 90,
            auto_cleanup: true,
            compression_enabled: true,
        }
    }
}

impl RetentionPolicy {
    pub fn validate(&self, name: &str) -> RavenResult<()> {
        if self.full_resolution_days == 0 {
            return Err(RavenError::invalid_config_value(
                format!("{name}.full_resolution_days"),
                self.full_resolution_days.to_string(),
            ));
        }

        if self.downsampled_days < self.full_resolution_days {
            return Err(RavenError::configuration(format!(
                "{name}: Downsampled days must be >= full resolution days"
            )));
        }

        if self.archive_days < self.downsampled_days {
            return Err(RavenError::configuration(format!(
                "{name}: Archive days must be >= downsampled days"
            )));
        }

        Ok(())
    }
}

impl ConfigSection for RetentionPolicies {
    const KEY: &'static str = "retention_policies";

    fn validate(&self) -> RavenResult<()> {
        self.high_frequency
            .validate("retention_policies.high_frequency")?;
        self.low_frequency
            .validate("retention_policies.low_frequency")?;
        self.private_data
            .validate("retention_policies.private_data")?;
        self.system_logs
            .validate("retention_policies.system_logs")?;
        Ok(())
    }
}

use serde::{Deserialize, Serialize};

use crate::common::config::validation::ConfigSection;
use crate::common::error::RavenResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionPolicies {
    pub high_frequency: RetentionPolicy,
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
            crate::raven_bail!(crate::raven_error!(
                invalid_config_value,
                format!("{name}.full_resolution_days"),
                self.full_resolution_days.to_string(),
            ));
        }

        if self.downsampled_days < self.full_resolution_days {
            crate::raven_bail!(crate::raven_error!(
                configuration,
                format!("{name}: Downsampled days must be >= full resolution days")
            ));
        }

        if self.archive_days < self.downsampled_days {
            crate::raven_bail!(crate::raven_error!(
                configuration,
                format!("{name}: Archive days must be >= downsampled days")
            ));
        }

        Ok(())
    }
}

impl ConfigSection for RetentionPolicies {
    const KEY: &'static str = "retention_policies";

    fn validate(&self) -> RavenResult<()> {
        self.high_frequency
            .validate("retention_policies.high_frequency")?;
        self.system_logs
            .validate("retention_policies.system_logs")?;
        Ok(())
    }
}

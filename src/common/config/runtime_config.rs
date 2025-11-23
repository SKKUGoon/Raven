use std::collections::HashMap;

use serde::Serialize;

use crate::common::config::loader::ConfigLoader;
use crate::common::config::sections::{
    DataProcessingConfig, DatabaseConfig, MonitoringConfig, ServerConfig,
};
use crate::common::config::validation::ConfigSection;
use crate::common::error::RavenResult;

#[derive(Debug, Clone, Serialize, Default)]
pub struct RuntimeConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub data_processing: DataProcessingConfig,
    pub monitoring: MonitoringConfig,
}

impl RuntimeConfig {
    pub fn load() -> RavenResult<Self> {
        Self::load_with_loader(&ConfigLoader::new())
    }

    pub fn load_with_loader(loader: &ConfigLoader) -> RavenResult<Self> {
        let server = loader.load_section::<ServerConfig>()?;
        let database = loader.load_section::<DatabaseConfig>()?;
        let data_processing = loader.load_section::<DataProcessingConfig>()?;
        let monitoring = loader.load_section::<MonitoringConfig>()?;

        let config = RuntimeConfig {
            server,
            database,
            data_processing,
            monitoring,
        };

        config.validate()?;

        Ok(config)
    }

    pub fn validate(&self) -> RavenResult<()> {
        self.server.validate()?;
        self.database.validate()?;
        self.data_processing.validate()?;
        self.monitoring.validate()?;
        Ok(())
    }

    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        vars.insert("RAVEN_SERVER__HOST".to_string(), self.server.host.clone());
        vars.insert(
            "RAVEN_SERVER__PORT".to_string(),
            self.server.port.to_string(),
        );
        vars.insert(
            "RAVEN_SERVER__MAX_CONNECTIONS".to_string(),
            self.server.max_connections.to_string(),
        );

        vars.insert(
            "RAVEN_DATABASE__INFLUX_URL".to_string(),
            self.database.influx_url.clone(),
        );
        vars.insert(
            "RAVEN_DATABASE__BUCKET".to_string(),
            self.database.bucket.clone(),
        );
        vars.insert("RAVEN_DATABASE__ORG".to_string(), self.database.org.clone());

        vars
    }
}

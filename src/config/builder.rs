use std::collections::HashMap;

use serde::Serialize;

use crate::config::loader::ConfigLoader;
use crate::config::sections::{
    BatchingConfig, DataProcessingConfig, DatabaseConfig, MonitoringConfig, RetentionPolicies,
    ServerConfig,
};
use crate::config::validation::ConfigSection;
use crate::error::RavenResult;

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub data_processing: DataProcessingConfig,
    pub retention: RetentionPolicies,
    pub batching: BatchingConfig,
    pub monitoring: MonitoringConfig,
}

impl RuntimeConfig {
    pub fn builder() -> RuntimeConfigBuilder {
        RuntimeConfigBuilder::new()
    }

    pub fn load() -> RavenResult<Self> {
        Self::builder()
            .hydrate_from_loader(&ConfigLoader::new())?
            .build()
    }

    pub fn load_with_loader(loader: &ConfigLoader) -> RavenResult<Self> {
        Self::builder().hydrate_from_loader(loader)?.build()
    }

    pub fn validate(&self) -> RavenResult<()> {
        self.server.validate()?;
        self.database.validate()?;
        self.data_processing.validate()?;
        self.retention.validate()?;
        self.batching.validate()?;
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

impl Default for RuntimeConfig {
    fn default() -> Self {
        RuntimeConfig::builder()
            .build()
            .expect("default runtime configuration to be valid")
    }
}

#[derive(Debug, Default)]
pub struct RuntimeConfigBuilder {
    server: Option<ServerConfig>,
    database: Option<DatabaseConfig>,
    data_processing: Option<DataProcessingConfig>,
    retention: Option<RetentionPolicies>,
    batching: Option<BatchingConfig>,
    monitoring: Option<MonitoringConfig>,
}

impl RuntimeConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn server(mut self, config: ServerConfig) -> Self {
        self.server = Some(config);
        self
    }

    pub fn database(mut self, config: DatabaseConfig) -> Self {
        self.database = Some(config);
        self
    }

    pub fn data_processing(mut self, config: DataProcessingConfig) -> Self {
        self.data_processing = Some(config);
        self
    }

    pub fn retention(mut self, config: RetentionPolicies) -> Self {
        self.retention = Some(config);
        self
    }

    pub fn batching(mut self, config: BatchingConfig) -> Self {
        self.batching = Some(config);
        self
    }

    pub fn monitoring(mut self, config: MonitoringConfig) -> Self {
        self.monitoring = Some(config);
        self
    }

    pub fn hydrate_from_loader(mut self, loader: &ConfigLoader) -> RavenResult<Self> {
        self.server = Some(loader.load_section::<ServerConfig>()?);
        self.database = Some(loader.load_section::<DatabaseConfig>()?);
        self.data_processing = Some(loader.load_section::<DataProcessingConfig>()?);
        self.retention = Some(loader.load_section::<RetentionPolicies>()?);
        self.batching = Some(loader.load_section::<BatchingConfig>()?);
        self.monitoring = Some(loader.load_section::<MonitoringConfig>()?);
        Ok(self)
    }

    pub fn build(self) -> RavenResult<RuntimeConfig> {
        let server = self.server.unwrap_or_default();
        let database = self.database.unwrap_or_default();
        let data_processing = self.data_processing.unwrap_or_default();
        let retention = self.retention.unwrap_or_default();
        let batching = self.batching.unwrap_or_default();
        let monitoring = self.monitoring.unwrap_or_default();

        server.validate()?;
        database.validate()?;
        data_processing.validate()?;
        retention.validate()?;
        batching.validate()?;
        monitoring.validate()?;

        Ok(RuntimeConfig {
            server,
            database,
            data_processing,
            retention,
            batching,
            monitoring,
        })
    }
}

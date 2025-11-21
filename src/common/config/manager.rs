use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::fs;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::common::config::builder::RuntimeConfig;
use crate::common::config::loader::ConfigLoader;
use crate::common::config::sections::{
    BatchingConfig, DataProcessingConfig, DatabaseConfig, MonitoringConfig, RetentionPolicies,
    ServerConfig,
};
use crate::common::error::RavenResult;

pub struct ConfigManager {
    loader: ConfigLoader,
    runtime: Arc<RwLock<RuntimeConfig>>,
    last_modified: Arc<RwLock<Option<SystemTime>>>,
    reload_interval: Duration,
    config_path: Option<PathBuf>,
}

impl ConfigManager {
    pub fn new(loader: ConfigLoader, reload_interval: Duration) -> RavenResult<Self> {
        let runtime = RuntimeConfig::load_with_loader(&loader)?;
        let config_path = loader.config_path().filter(|path| path.exists());

        let last_modified = if let Some(path) = &config_path {
            Some(std::fs::metadata(path)?.modified()?)
        } else {
            None
        };

        Ok(Self {
            loader,
            runtime: Arc::new(RwLock::new(runtime)),
            last_modified: Arc::new(RwLock::new(last_modified)),
            reload_interval,
            config_path,
        })
    }

    pub async fn runtime(&self) -> RuntimeConfig {
        self.runtime.read().await.clone()
    }

    pub async fn server(&self) -> ServerConfig {
        self.runtime.read().await.server.clone()
    }

    pub async fn database(&self) -> DatabaseConfig {
        self.runtime.read().await.database.clone()
    }

    pub async fn data_processing(&self) -> DataProcessingConfig {
        self.runtime.read().await.data_processing.clone()
    }

    pub async fn batching(&self) -> BatchingConfig {
        self.runtime.read().await.batching.clone()
    }

    pub async fn retention(&self) -> RetentionPolicies {
        self.runtime.read().await.retention.clone()
    }

    pub async fn monitoring(&self) -> MonitoringConfig {
        self.runtime.read().await.monitoring.clone()
    }

    pub async fn start_hot_reload(&self) -> RavenResult<()> {
        let runtime = Arc::clone(&self.runtime);
        let loader = self.loader.clone();
        let last_modified = Arc::clone(&self.last_modified);
        let config_path = self.config_path.clone();
        let mut interval = interval(self.reload_interval);

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                if let Err(err) =
                    Self::check_and_reload(&loader, &runtime, &last_modified, config_path.clone())
                        .await
                {
                    error!("Failed to reload configuration: {}", err);
                }
            }
        });

        info!(
            "⟲ Configuration hot-reloading started (interval: {:?})",
            self.reload_interval
        );
        Ok(())
    }

    pub async fn force_reload(&self) -> RavenResult<()> {
        info!("⟲ Force reloading configuration...");
        let config = RuntimeConfig::load_with_loader(&self.loader)?;
        *self.runtime.write().await = config;

        if let Some(path) = self.config_path.as_ref() {
            if path.exists() {
                let metadata = std::fs::metadata(path)?;
                *self.last_modified.write().await = Some(metadata.modified()?);
            }
        }

        info!("✓ Configuration force reloaded successfully");
        Ok(())
    }

    async fn check_and_reload(
        loader: &ConfigLoader,
        runtime: &Arc<RwLock<RuntimeConfig>>,
        last_modified: &Arc<RwLock<Option<SystemTime>>>,
        config_path: Option<PathBuf>,
    ) -> RavenResult<()> {
        let Some(path) = config_path else {
            return Ok(());
        };

        if !path.exists() {
            warn!(
                "Configuration file {} disappeared; skipping reload",
                path.display()
            );
            return Ok(());
        }

        let metadata = fs::metadata(&path).await?;
        let modified = metadata.modified()?;

        let should_reload = {
            let current = last_modified.read().await;
            current.map(|ts| modified > ts).unwrap_or(true)
        };

        if should_reload {
            info!("⚬ Configuration file changed, reloading...");
            match RuntimeConfig::load_with_loader(loader) {
                Ok(config) => {
                    *runtime.write().await = config;
                    *last_modified.write().await = Some(modified);
                    info!("✓ Configuration reloaded successfully");
                }
                Err(err) => {
                    error!("✗ Failed to reload configuration: {}", err);
                    warn!("⟲ Keeping current configuration");
                }
            }
        }

        Ok(())
    }
}

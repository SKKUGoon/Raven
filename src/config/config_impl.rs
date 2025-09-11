// Configuration Management - Project Raven
// "The rules and settings that govern the realm, as flexible as the wind"

use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, Environment, File};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::fs;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info, warn};

/// Main configuration structure for the market data subscription server
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub data_processing: DataProcessingConfig,
    pub retention_policies: RetentionPolicies,
    pub batching: BatchingConfig,
    pub monitoring: MonitoringConfig,
}

/// Server configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub heartbeat_interval_seconds: u64,
    pub client_timeout_seconds: u64,
    pub enable_compression: bool,
    pub max_message_size: usize,
}

/// Database connection and configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub influx_url: String,
    pub database_name: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub connection_pool_size: usize,
    pub connection_timeout_seconds: u64,
    pub write_timeout_seconds: u64,
    pub query_timeout_seconds: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_seconds: u64,
}

/// Data processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataProcessingConfig {
    pub snapshot_interval_ms: u64,
    pub high_frequency_buffer_size: usize,
    pub low_frequency_buffer_size: usize,
    pub private_data_buffer_size: usize,
    pub data_validation_enabled: bool,
    pub sanitization_enabled: bool,
    pub dead_letter_queue_size: usize,
}

/// Retention policies for different data types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicies {
    pub high_frequency: RetentionPolicy,
    pub low_frequency: RetentionPolicy,
    pub private_data: RetentionPolicy,
    pub system_logs: RetentionPolicy,
}

/// Individual retention policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub full_resolution_days: u32,
    pub downsampled_days: u32,
    pub archive_days: u32,
    pub auto_cleanup: bool,
    pub compression_enabled: bool,
}

/// Batching configuration for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchingConfig {
    pub database_writes: BatchConfig,
    pub client_broadcasts: BatchConfig,
    pub snapshot_captures: BatchConfig,
    pub metrics_collection: BatchConfig,
}

/// Individual batch configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    pub size: usize,
    pub timeout_ms: u64,
    pub max_memory_mb: usize,
    pub compression_threshold: usize,
}

/// Monitoring and observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub metrics_enabled: bool,
    pub metrics_port: u16,
    pub health_check_port: u16,
    pub tracing_enabled: bool,
    pub log_level: String,
    pub performance_monitoring: bool,
}

/// Configuration manager with hot-reloading capability
pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    config_path: String,
    last_modified: Arc<RwLock<SystemTime>>,
    reload_interval: Duration,
}

// Using derive(Default) instead of manual implementation
// This is more idiomatic and avoids clippy warnings

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
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

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig {
            influx_url: "http://localhost:8086".to_string(),
            database_name: "market_data".to_string(),
            username: None,
            password: None,
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

impl Default for DataProcessingConfig {
    fn default() -> Self {
        DataProcessingConfig {
            snapshot_interval_ms: 5,
            high_frequency_buffer_size: 10000,
            low_frequency_buffer_size: 1000,
            private_data_buffer_size: 500,
            data_validation_enabled: true,
            sanitization_enabled: true,
            dead_letter_queue_size: 1000,
        }
    }
}

impl Default for RetentionPolicies {
    fn default() -> Self {
        RetentionPolicies {
            high_frequency: RetentionPolicy {
                full_resolution_days: 7,
                downsampled_days: 30,
                archive_days: 90,
                auto_cleanup: true,
                compression_enabled: true,
            },
            low_frequency: RetentionPolicy {
                full_resolution_days: 730, // 2 years
                downsampled_days: 1095,    // 3 years
                archive_days: 1825,        // 5 years
                auto_cleanup: true,
                compression_enabled: true,
            },
            private_data: RetentionPolicy {
                full_resolution_days: 365, // 1 year
                downsampled_days: 730,     // 2 years
                archive_days: 1095,        // 3 years
                auto_cleanup: false,       // Manual cleanup for sensitive data
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

impl Default for BatchingConfig {
    fn default() -> Self {
        BatchingConfig {
            database_writes: BatchConfig {
                size: 1000,
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
                timeout_ms: 1000,
                max_memory_mb: 10,
                compression_threshold: 250,
            },
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        MonitoringConfig {
            metrics_enabled: true,
            metrics_port: 9090,
            health_check_port: 8080,
            tracing_enabled: true,
            log_level: "info".to_string(),
            performance_monitoring: true,
        }
    }
}

impl Config {
    /// Load configuration from environment variables and config files
    pub fn load() -> Result<Self> {
        info!("📜 Loading configuration for the realm...");

        let mut builder = ConfigBuilder::builder()
            // Start with default values
            .add_source(config::Config::try_from(&Config::default())?)
            // Add config file if it exists
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/local").required(false));

        // Add environment-specific config file
        if let Ok(env) = std::env::var("ENVIRONMENT") {
            builder = builder.add_source(File::with_name(&format!("config/{env}")).required(false));
        }

        // Add environment variables with prefix
        builder = builder.add_source(
            Environment::with_prefix("RAVEN")
                .prefix_separator("_")
                .separator("__"),
        );

        let config = builder
            .build()
            .context("Failed to build configuration")?
            .try_deserialize::<Config>()
            .context("Failed to deserialize configuration")?;

        // Validate configuration
        config.validate()?;

        info!("✅ Configuration loaded successfully");
        Ok(config)
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        info!("🔍 Validating configuration...");

        // Validate server configuration
        if self.server.port == 0 {
            return Err(anyhow::anyhow!("Server port cannot be 0"));
        }
        if self.server.max_connections == 0 {
            return Err(anyhow::anyhow!("Max connections must be greater than 0"));
        }
        if self.server.max_message_size == 0 {
            return Err(anyhow::anyhow!("Max message size must be greater than 0"));
        }

        // Validate database configuration
        if self.database.influx_url.is_empty() {
            return Err(anyhow::anyhow!("InfluxDB URL cannot be empty"));
        }
        if self.database.database_name.is_empty() {
            return Err(anyhow::anyhow!("Database name cannot be empty"));
        }
        if self.database.connection_pool_size == 0 {
            return Err(anyhow::anyhow!(
                "Connection pool size must be greater than 0"
            ));
        }

        // Validate data processing configuration
        if self.data_processing.snapshot_interval_ms == 0 {
            return Err(anyhow::anyhow!("Snapshot interval must be greater than 0"));
        }

        // Validate retention policies
        self.retention_policies.validate()?;

        // Validate batching configuration
        self.batching.validate()?;

        // Validate monitoring configuration
        if !["trace", "debug", "info", "warn", "error"]
            .contains(&self.monitoring.log_level.as_str())
        {
            return Err(anyhow::anyhow!(
                "Invalid log level: {}",
                self.monitoring.log_level
            ));
        }

        info!("✅ Configuration validation passed");
        Ok(())
    }

    /// Get configuration as environment variables map for debugging
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        // Server config
        vars.insert("RAVEN_SERVER__HOST".to_string(), self.server.host.clone());
        vars.insert(
            "RAVEN_SERVER__PORT".to_string(),
            self.server.port.to_string(),
        );
        vars.insert(
            "RAVEN_SERVER__MAX_CONNECTIONS".to_string(),
            self.server.max_connections.to_string(),
        );

        // Database config
        vars.insert(
            "RAVEN_DATABASE__INFLUX_URL".to_string(),
            self.database.influx_url.clone(),
        );
        vars.insert(
            "RAVEN_DATABASE__DATABASE_NAME".to_string(),
            self.database.database_name.clone(),
        );

        // Add more as needed for debugging
        vars
    }
}

impl RetentionPolicies {
    pub fn validate(&self) -> Result<()> {
        self.high_frequency.validate("high_frequency")?;
        self.low_frequency.validate("low_frequency")?;
        self.private_data.validate("private_data")?;
        self.system_logs.validate("system_logs")?;
        Ok(())
    }
}

impl RetentionPolicy {
    pub fn validate(&self, name: &str) -> Result<()> {
        if self.full_resolution_days == 0 {
            return Err(anyhow::anyhow!(
                "{}: Full resolution days must be greater than 0",
                name
            ));
        }
        if self.downsampled_days < self.full_resolution_days {
            return Err(anyhow::anyhow!(
                "{}: Downsampled days must be >= full resolution days",
                name
            ));
        }
        if self.archive_days < self.downsampled_days {
            return Err(anyhow::anyhow!(
                "{}: Archive days must be >= downsampled days",
                name
            ));
        }
        Ok(())
    }
}

impl BatchingConfig {
    pub fn validate(&self) -> Result<()> {
        self.database_writes.validate("database_writes")?;
        self.client_broadcasts.validate("client_broadcasts")?;
        self.snapshot_captures.validate("snapshot_captures")?;
        self.metrics_collection.validate("metrics_collection")?;
        Ok(())
    }
}

impl BatchConfig {
    pub fn validate(&self, name: &str) -> Result<()> {
        if self.size == 0 {
            return Err(anyhow::anyhow!(
                "{}: Batch size must be greater than 0",
                name
            ));
        }
        if self.timeout_ms == 0 {
            return Err(anyhow::anyhow!(
                "{}: Batch timeout must be greater than 0",
                name
            ));
        }
        if self.max_memory_mb == 0 {
            return Err(anyhow::anyhow!(
                "{}: Max memory must be greater than 0",
                name
            ));
        }
        Ok(())
    }
}

impl ConfigManager {
    /// Create a new configuration manager with hot-reloading capability
    pub fn new(config_path: String, reload_interval: Duration) -> Result<Self> {
        let config = Config::load()?;
        let last_modified = if Path::new(&config_path).exists() {
            std::fs::metadata(&config_path)?.modified()?
        } else {
            SystemTime::now()
        };

        Ok(ConfigManager {
            config: Arc::new(RwLock::new(config)),
            config_path,
            last_modified: Arc::new(RwLock::new(last_modified)),
            reload_interval,
        })
    }

    /// Get current configuration
    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    /// Start hot-reloading background task
    pub async fn start_hot_reload(&self) -> Result<()> {
        let config = Arc::clone(&self.config);
        let last_modified = Arc::clone(&self.last_modified);
        let config_path = self.config_path.clone();
        let mut interval = interval(self.reload_interval);

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                if let Err(e) = Self::check_and_reload(&config, &last_modified, &config_path).await
                {
                    error!("Failed to reload configuration: {}", e);
                }
            }
        });

        info!(
            "🔄 Configuration hot-reloading started (interval: {:?})",
            self.reload_interval
        );
        Ok(())
    }

    /// Check if config file has changed and reload if necessary
    async fn check_and_reload(
        config: &Arc<RwLock<Config>>,
        last_modified: &Arc<RwLock<SystemTime>>,
        config_path: &str,
    ) -> Result<()> {
        if !Path::new(config_path).exists() {
            return Ok(());
        }

        let metadata = fs::metadata(config_path).await?;
        let current_modified = metadata.modified()?;
        let last_mod = *last_modified.read().await;

        if current_modified > last_mod {
            info!("📝 Configuration file changed, reloading...");

            match Config::load() {
                Ok(new_config) => {
                    *config.write().await = new_config;
                    *last_modified.write().await = current_modified;
                    info!("✅ Configuration reloaded successfully");
                }
                Err(e) => {
                    error!("❌ Failed to reload configuration: {}", e);
                    warn!("🔄 Keeping current configuration");
                }
            }
        }

        Ok(())
    }

    /// Force reload configuration
    pub async fn force_reload(&self) -> Result<()> {
        info!("🔄 Force reloading configuration...");
        let new_config = Config::load()?;
        *self.config.write().await = new_config;

        if Path::new(&self.config_path).exists() {
            let metadata = std::fs::metadata(&self.config_path)?;
            *self.last_modified.write().await = metadata.modified()?;
        }

        info!("✅ Configuration force reloaded successfully");
        Ok(())
    }
}

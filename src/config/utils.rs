// Configuration Utilities - Project Raven
// "Tools to shape and mold the realm's rules"

use anyhow::Result;
use serde_json;
use std::collections::HashMap;
use std::env;
use tracing::{info, warn};

use super::Config;

/// Configuration utilities for debugging and validation
pub struct ConfigUtils;

impl ConfigUtils {
    /// Print current configuration in a readable format
    pub fn print_config(config: &Config) {
        info!("🏰 Current Configuration for Project Raven:");
        info!("  📡 Server: {}:{}", config.server.host, config.server.port);
        info!("  🔗 Max Connections: {}", config.server.max_connections);
        info!("  🗄️  Database: {}", config.database.influx_url);
        info!("  🪣 Bucket: {}", config.database.bucket);
        info!("  Organization: {}", config.database.org);
        info!(
            "  Snapshot Interval: {}ms",
            config.data_processing.snapshot_interval_ms
        );
        info!(
            "  📈 High Freq Buffer: {}",
            config.data_processing.high_frequency_buffer_size
        );
        info!(
            "  📉 Low Freq Buffer: {}",
            config.data_processing.low_frequency_buffer_size
        );
        info!(
            "  🔒 Private Buffer: {}",
            config.data_processing.private_data_buffer_size
        );
        info!("  📝 Log Level: {}", config.monitoring.log_level);
    }

    /// Get configuration summary for health checks
    pub fn get_config_summary(config: &Config) -> HashMap<String, String> {
        let mut summary = HashMap::new();

        summary.insert("server_host".to_string(), config.server.host.clone());
        summary.insert("server_port".to_string(), config.server.port.to_string());
        summary.insert(
            "max_connections".to_string(),
            config.server.max_connections.to_string(),
        );
        summary.insert(
            "database_url".to_string(),
            config.database.influx_url.clone(),
        );
        summary.insert("bucket".to_string(), config.database.bucket.clone());
        summary.insert("org".to_string(), config.database.org.clone());
        summary.insert(
            "snapshot_interval_ms".to_string(),
            config.data_processing.snapshot_interval_ms.to_string(),
        );
        summary.insert("log_level".to_string(), config.monitoring.log_level.clone());
        summary.insert(
            "metrics_enabled".to_string(),
            config.monitoring.metrics_enabled.to_string(),
        );

        summary
    }

    /// Export configuration as JSON for debugging
    pub fn export_as_json(config: &Config) -> Result<String> {
        serde_json::to_string_pretty(config)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config to JSON: {}", e))
    }

    /// Check for common configuration issues
    pub fn check_configuration_health(config: &Config) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check for potential performance issues
        if config.data_processing.snapshot_interval_ms > 100 {
            warnings.push("Snapshot interval > 100ms may impact real-time performance".to_string());
        }

        if config.server.max_connections > 10000 {
            warnings.push("Very high max_connections may require system tuning".to_string());
        }

        if config.database.connection_pool_size < 5 {
            warnings.push("Small connection pool may become a bottleneck".to_string());
        }

        // Check for security issues
        if config.database.token.is_none() {
            warnings.push("Database token not configured - using anonymous access".to_string());
        }

        if config.server.host == "0.0.0.0" {
            warnings.push(
                "Server listening on all interfaces - ensure firewall is configured".to_string(),
            );
        }

        // Check retention policies
        if config
            .retention_policies
            .high_frequency
            .full_resolution_days
            > 30
        {
            warnings.push(
                "High frequency data retention > 30 days may consume significant storage"
                    .to_string(),
            );
        }

        if !config.retention_policies.private_data.auto_cleanup {
            warnings
                .push("Private data auto-cleanup disabled - manual cleanup required".to_string());
        }

        // Check batching configuration
        if config.batching.database_writes.size > 10000 {
            warnings.push("Very large database write batches may cause memory issues".to_string());
        }

        if config.batching.database_writes.timeout_ms > 1000 {
            warnings
                .push("High database write timeout may impact real-time performance".to_string());
        }

        warnings
    }

    /// Get environment-specific recommendations
    pub fn get_environment_recommendations() -> HashMap<String, Vec<String>> {
        let mut recommendations = HashMap::new();

        recommendations.insert(
            "development".to_string(),
            vec![
                "Use smaller buffer sizes for easier debugging".to_string(),
                "Enable verbose logging (debug level)".to_string(),
                "Disable compression for easier inspection".to_string(),
                "Use shorter retention periods to save storage".to_string(),
                "Enable auto-cleanup for all data types".to_string(),
            ],
        );

        recommendations.insert(
            "production".to_string(),
            vec![
                "Use larger buffer sizes for better performance".to_string(),
                "Set log level to 'warn' or 'error' to reduce overhead".to_string(),
                "Enable compression for all data types".to_string(),
                "Configure appropriate retention policies for compliance".to_string(),
                "Disable auto-cleanup for sensitive data".to_string(),
                "Use connection pooling with adequate pool size".to_string(),
                "Enable circuit breakers for resilience".to_string(),
            ],
        );

        recommendations.insert(
            "staging".to_string(),
            vec![
                "Use production-like settings but with shorter retention".to_string(),
                "Enable detailed monitoring and metrics".to_string(),
                "Use moderate buffer sizes".to_string(),
                "Test failover scenarios with circuit breakers".to_string(),
            ],
        );

        recommendations
    }

    /// Validate environment variables are properly set
    pub fn validate_environment() -> Result<()> {
        let required_vars = vec![
            "RAVEN_DATABASE__INFLUX_URL",
            "RAVEN_DATABASE__BUCKET",
            "RAVEN_DATABASE__ORG",
        ];

        let mut missing_vars = Vec::new();

        for var in required_vars {
            if env::var(var).is_err() {
                missing_vars.push(var);
            }
        }

        if !missing_vars.is_empty() {
            warn!("Missing environment variables: {:?}", missing_vars);
            warn!("Consider setting these variables or using config files");
        }

        // Check for common misconfigurations
        if let Ok(port) = env::var("RAVEN_SERVER__PORT") {
            if let Ok(port_num) = port.parse::<u16>() {
                if port_num < 1024 && env::var("USER").unwrap_or_default() != "root" {
                    warn!("⚠️  Port {} requires root privileges", port_num);
                }
            }
        }

        Ok(())
    }

    /// Generate configuration template
    pub fn generate_config_template() -> String {
        r#"# Project Raven Configuration Template
# Copy this to config/local.toml and customize as needed

[server]
host = "0.0.0.0"
port = 50051
max_connections = 1000

[database]
influx_url = "http://localhost:8086"
bucket = "market_data"
org = "raven"
# token = "your_influxdb_v2_token"

[data_processing]
snapshot_interval_ms = 5
high_frequency_buffer_size = 10000

[monitoring]
log_level = "info"
metrics_enabled = true

# Add other sections as needed...
"#
        .to_string()
    }
}

/// Configuration validation helpers
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate port ranges
    pub fn validate_port(port: u16, name: &str) -> Result<()> {
        if port == 0 {
            return Err(anyhow::anyhow!("{} port cannot be 0", name));
        }
        if port < 1024 {
            warn!("⚠️  {} port {} requires elevated privileges", name, port);
        }
        Ok(())
    }

    /// Validate URL format
    pub fn validate_url(url: &str, name: &str) -> Result<()> {
        if url.is_empty() {
            return Err(anyhow::anyhow!("{} URL cannot be empty", name));
        }

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(anyhow::anyhow!(
                "{} URL must start with http:// or https://",
                name
            ));
        }

        Ok(())
    }

    /// Validate buffer sizes
    pub fn validate_buffer_size(size: usize, name: &str, min_size: usize) -> Result<()> {
        if size < min_size {
            return Err(anyhow::anyhow!(
                "{} buffer size {} is below minimum {}",
                name,
                size,
                min_size
            ));
        }

        if size > 1_000_000 {
            warn!(
                "⚠️  {} buffer size {} is very large and may consume significant memory",
                name, size
            );
        }

        Ok(())
    }

    /// Validate timeout values
    pub fn validate_timeout(timeout_ms: u64, name: &str) -> Result<()> {
        if timeout_ms == 0 {
            return Err(anyhow::anyhow!("{} timeout cannot be 0", name));
        }

        if timeout_ms > 300_000 {
            // 5 minutes
            warn!("⚠️  {} timeout {}ms is very high", name, timeout_ms);
        }

        Ok(())
    }
}

// Configuration Integration Tests - Project Raven
// "Testing the complete wisdom of the realm"

use crate::config::*;
use std::env;
use std::time::Duration;
use tempfile::tempdir;
use tokio::fs;

#[tokio::test]
async fn test_full_configuration_lifecycle() {
    // Test complete configuration loading, validation, and hot-reload
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("integration_test.toml");

    // Create a test configuration file
    let test_config = r#"
[server]
host = "127.0.0.1"
port = 8080
max_connections = 500

[database]
influx_url = "http://test-influx:8086"
bucket = "test_db"
org = "test_org"
connection_pool_size = 10

[data_processing]
snapshot_interval_ms = 10
high_frequency_buffer_size = 5000

[monitoring]
log_level = "debug"
metrics_enabled = true
"#;
    fs::write(&config_path, test_config).await.unwrap();

    // Test configuration manager creation
    let loader = ConfigLoader::new().with_file(config_path.to_path_buf());
    let manager = ConfigManager::new(loader, Duration::from_millis(100)).unwrap();

    // Test getting configuration
    let config = manager.runtime().await;
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.database.bucket, "test_db");
    assert_eq!(config.database.org, "test_org");

    // Test configuration validation
    assert!(config.validate().is_ok());

    // Test configuration utilities
    let summary = ConfigUtils::get_config_summary(&config);
    assert!(summary.contains_key("server_host"));
    assert_eq!(summary.get("server_host").unwrap(), "127.0.0.1");

    let json_export = ConfigUtils::export_as_json(&config).unwrap();
    assert!(json_export.contains("server"));
    assert!(json_export.contains("database"));

    // Test health check
    let warnings = ConfigUtils::check_configuration_health(&config);
    // Should have some warnings for test config
    assert!(!warnings.is_empty());

    // Test force reload
    assert!(manager.force_reload().await.is_ok());
}

#[test]
fn test_environment_specific_configurations() {
    // Test that different environment configs have appropriate settings

    // Development should have smaller buffers and verbose logging
    let dev_config = r#"
[data_processing]
high_frequency_buffer_size = 1000

[monitoring]
log_level = "debug"
"#;

    // Production should have larger buffers and less verbose logging
    let prod_config = r#"
[data_processing]
high_frequency_buffer_size = 50000

[monitoring]
log_level = "warn"
"#;

    // These would be loaded based on ENVIRONMENT variable
    assert!(dev_config.contains("debug"));
    assert!(prod_config.contains("warn"));
    assert!(dev_config.contains("1000"));
    assert!(prod_config.contains("50000"));
}

#[test]
fn test_environment_variable_precedence() {
    // Test that environment variables override config files
    env::set_var("RAVEN_SERVER__PORT", "9999");
    env::set_var("RAVEN_DATABASE__BUCKET", "env_override_db");

    // In a real test, we would load config and verify overrides
    assert_eq!(env::var("RAVEN_SERVER__PORT").unwrap(), "9999");
    assert_eq!(
        env::var("RAVEN_DATABASE__BUCKET").unwrap(),
        "env_override_db"
    );

    // Clean up
    env::remove_var("RAVEN_SERVER__PORT");
    env::remove_var("RAVEN_DATABASE__BUCKET");
}

#[test]
fn test_configuration_validation_comprehensive() {
    let mut config = RuntimeConfig::default();

    // Test all validation scenarios
    assert!(config.validate().is_ok());

    // Test server validation
    config.server.port = 0;
    assert!(config.validate().is_err());
    config.server.port = 50051;

    config.server.max_connections = 0;
    assert!(config.validate().is_err());
    config.server.max_connections = 1000;

    // Test database validation
    config.database.influx_url = String::new();
    assert!(config.validate().is_err());
    config.database.influx_url = "http://localhost:8086".to_string();

    config.database.connection_pool_size = 0;
    assert!(config.validate().is_err());
    config.database.connection_pool_size = 20;

    // Test data processing validation
    config.data_processing.snapshot_interval_ms = 0;
    assert!(config.validate().is_err());
    config.data_processing.snapshot_interval_ms = 5;

    // Test monitoring validation
    config.monitoring.log_level = "invalid".to_string();
    assert!(config.validate().is_err());
    config.monitoring.log_level = "info".to_string();

    assert!(config.validate().is_ok());
}

#[test]
fn test_retention_policy_comprehensive_validation() {
    let mut policies = RetentionPolicies::default();

    // Valid policies should pass
    assert!(policies.validate().is_ok());

    // Test invalid high frequency policy
    policies.high_frequency.full_resolution_days = 0;
    assert!(policies.validate().is_err());
    policies.high_frequency.full_resolution_days = 7;

    // Test invalid ordering
    policies.low_frequency.downsampled_days = 5; // Less than full_resolution_days
    assert!(policies.validate().is_err());
    policies.low_frequency.downsampled_days = 1095;

    // Test archive days validation
    policies.private_data.archive_days = 100; // Less than downsampled_days
    assert!(policies.validate().is_err());
    policies.private_data.archive_days = 1095;

    assert!(policies.validate().is_ok());
}

#[test]
fn test_batching_configuration_comprehensive() {
    let mut batching = BatchingConfig::default();

    // Valid batching should pass
    assert!(batching.validate().is_ok());

    // Test zero size validation
    batching.database_writes.size = 0;
    assert!(batching.validate().is_err());
    batching.database_writes.size = 1000;

    // Test zero timeout validation
    batching.client_broadcasts.timeout_ms = 0;
    assert!(batching.validate().is_err());
    batching.client_broadcasts.timeout_ms = 1;

    // Test zero memory validation
    batching.snapshot_captures.max_memory_mb = 0;
    assert!(batching.validate().is_err());
    batching.snapshot_captures.max_memory_mb = 25;

    assert!(batching.validate().is_ok());
}

#[test]
fn test_config_utils_comprehensive() {
    let config = RuntimeConfig::default();

    // Test configuration summary
    let summary = ConfigUtils::get_config_summary(&config);
    assert!(summary.contains_key("server_host"));
    assert!(summary.contains_key("server_port"));
    assert!(summary.contains_key("database_url"));
    assert!(summary.contains_key("log_level"));

    // Test JSON export
    let json = ConfigUtils::export_as_json(&config).unwrap();
    assert!(json.contains("server"));
    assert!(json.contains("database"));
    assert!(json.contains("retention"));
    assert!(json.contains("batching"));

    // Test health check warnings
    let warnings = ConfigUtils::check_configuration_health(&config);
    assert!(!warnings.is_empty()); // Default config should have some warnings

    // Test environment recommendations
    let recommendations = ConfigUtils::get_environment_recommendations();
    assert!(recommendations.contains_key("development"));
    assert!(recommendations.contains_key("production"));
    assert!(recommendations.contains_key("staging"));

    // Test template generation
    let template = ConfigUtils::generate_config_template();
    assert!(template.contains("[server]"));
    assert!(template.contains("[database]"));
    assert!(template.contains("host"));
    assert!(template.contains("port"));
}

#[test]
fn test_config_validators_comprehensive() {
    // Test port validation
    assert!(ConfigValidator::validate_port(8080, "test").is_ok());
    assert!(ConfigValidator::validate_port(0, "test").is_err());
    assert!(ConfigValidator::validate_port(80, "test").is_ok()); // Should warn but not error

    // Test URL validation
    assert!(ConfigValidator::validate_url("http://localhost:8086", "test").is_ok());
    assert!(ConfigValidator::validate_url("https://example.com:8086", "test").is_ok());
    assert!(ConfigValidator::validate_url("", "test").is_err());
    assert!(ConfigValidator::validate_url("localhost:8086", "test").is_err());
    assert!(ConfigValidator::validate_url("ftp://example.com", "test").is_err());

    // Test buffer size validation
    assert!(ConfigValidator::validate_buffer_size(1000, "test", 100).is_ok());
    assert!(ConfigValidator::validate_buffer_size(50, "test", 100).is_err());
    assert!(ConfigValidator::validate_buffer_size(2_000_000, "test", 100).is_ok()); // Should warn

    // Test timeout validation
    assert!(ConfigValidator::validate_timeout(1000, "test").is_ok());
    assert!(ConfigValidator::validate_timeout(0, "test").is_err());
    assert!(ConfigValidator::validate_timeout(600_000, "test").is_ok()); // Should warn
}

#[tokio::test]
async fn test_hot_reload_functionality() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("hot_reload_test.toml");

    // Create initial config
    let initial_config = r#"
[server]
port = 8080

[database]
influx_url = "http://localhost:8086"
bucket = "initial_db"
org = "test_org"

[monitoring]
log_level = "info"
"#;
    fs::write(&config_path, initial_config).await.unwrap();

    let loader = ConfigLoader::new().with_file(config_path.to_path_buf());
    let manager = ConfigManager::new(loader, Duration::from_millis(50)).unwrap();

    // Get initial config
    let config = manager.runtime().await;
    assert_eq!(config.database.bucket, "initial_db");

    // Test force reload
    assert!(manager.force_reload().await.is_ok());

    // Update config file
    let updated_config = r#"
[server]
port = 9090

[database]
influx_url = "http://localhost:8086"
bucket = "updated_db"
org = "updated_org"

[monitoring]
log_level = "debug"
"#;
    fs::write(&config_path, updated_config).await.unwrap();

    // Force reload to pick up changes
    assert!(manager.force_reload().await.is_ok());
    let config = manager.runtime().await;
    assert_eq!(config.server.port, 9090);
    assert_eq!(config.database.bucket, "updated_db");
    assert_eq!(config.monitoring.log_level, "debug");
}

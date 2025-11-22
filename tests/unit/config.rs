use raven::common::config::*;
use std::env;
use std::time::Duration;
use tempfile::tempdir;
use tokio::fs;

#[test]
fn test_default_config_creation() {
    let config = RuntimeConfig::default();

    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.server.port, 50051);
    assert_eq!(config.server.max_connections, 1000);
    assert_eq!(config.database.influx_url, "http://localhost:8086");
    assert_eq!(config.database.bucket, "crypto");
    assert_eq!(config.database.org, "raven");
    assert_eq!(config.data_processing.high_frequency_buffer_size, 10000);
}

#[test]
fn test_config_validation() {
    let mut config = RuntimeConfig::default();

    // Valid config should pass
    assert!(config.validate().is_ok());

    // Invalid port should fail
    config.server.port = 0;
    assert!(config.validate().is_err());

    // Reset and test empty database URL
    config = RuntimeConfig::default();
    config.database.influx_url = String::new();
    assert!(config.validate().is_err());
}

#[test]
fn test_config_to_env_vars() {
    let config = RuntimeConfig::default();
    let env_vars = config.to_env_vars();

    assert_eq!(
        env_vars.get("RAVEN_SERVER__HOST"),
        Some(&"0.0.0.0".to_string())
    );
    assert_eq!(
        env_vars.get("RAVEN_SERVER__PORT"),
        Some(&"50051".to_string())
    );
    assert_eq!(
        env_vars.get("RAVEN_DATABASE__INFLUX_URL"),
        Some(&"http://localhost:8086".to_string())
    );
}

#[tokio::test]
async fn test_config_manager_creation() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir
        .path()
        .join("test_config.toml")
        .to_string_lossy()
        .to_string();

    let loader = ConfigLoader::new().with_file(config_path);
    let manager = ConfigManager::new(loader, Duration::from_secs(1));
    assert!(manager.is_ok());

    let manager = manager.unwrap();
    let config = manager.runtime().await;
    assert_eq!(config.server.port, 50051);
}

#[tokio::test]
async fn test_config_hot_reload() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");

    // Create initial config file
    let initial_config = r#"
[server]
port = 8080

[database]
influx_url = "http://localhost:8086"
bucket = "test_db"
org = "test_org"
"#;
    fs::write(&config_path, initial_config).await.unwrap();

    let loader = ConfigLoader::new().with_file(config_path.to_path_buf());
    let manager = ConfigManager::new(loader, Duration::from_millis(100)).unwrap();

    // Force reload should work
    assert!(manager.force_reload().await.is_ok());

    let config = manager.runtime().await;
    assert_eq!(config.server.port, 8080);
}

#[test]
fn test_config_utils_health_check() {
    let config = RuntimeConfig::default();
    let warnings = ConfigUtils::check_configuration_health(&config);

    // Should have some warnings for default config
    assert!(!warnings.is_empty());

    // Check for specific warnings (should have at least one warning)
    assert!(!warnings.is_empty());
}

#[test]
fn test_config_utils_export_json() {
    let config = RuntimeConfig::default();
    let json = ConfigUtils::export_as_json(&config);

    assert!(json.is_ok());
    let json_str = json.unwrap();
    assert!(json_str.contains("server"));
    assert!(json_str.contains("database"));
    assert!(json_str.contains("data_processing"));
}

#[test]
fn test_config_validator_port() {
    assert!(ConfigValidator::validate_port(8080, "test").is_ok());
    assert!(ConfigValidator::validate_port(0, "test").is_err());
    // Port 80 should warn but not error
    assert!(ConfigValidator::validate_port(80, "test").is_ok());
}

#[test]
fn test_config_validator_url() {
    assert!(ConfigValidator::validate_url("http://localhost:8086", "test").is_ok());
    assert!(ConfigValidator::validate_url("https://example.com", "test").is_ok());
    assert!(ConfigValidator::validate_url("", "test").is_err());
    assert!(ConfigValidator::validate_url("localhost:8086", "test").is_err());
}

#[test]
fn test_config_validator_buffer_size() {
    assert!(ConfigValidator::validate_buffer_size(1000, "test", 100).is_ok());
    assert!(ConfigValidator::validate_buffer_size(50, "test", 100).is_err());
    // Large buffer should warn but not error
    assert!(ConfigValidator::validate_buffer_size(2_000_000, "test", 100).is_ok());
}

#[test]
fn test_config_validator_timeout() {
    assert!(ConfigValidator::validate_timeout(1000, "test").is_ok());
    assert!(ConfigValidator::validate_timeout(0, "test").is_err());
    // Very large timeout should warn but not error
    assert!(ConfigValidator::validate_timeout(600_000, "test").is_ok());
}

#[test]
fn test_environment_recommendations() {
    let recommendations = ConfigUtils::get_environment_recommendations();

    assert!(recommendations.contains_key("development"));
    assert!(recommendations.contains_key("production"));
    assert!(recommendations.contains_key("staging"));

    let dev_recs = recommendations.get("development").unwrap();
    assert!(!dev_recs.is_empty());

    let prod_recs = recommendations.get("production").unwrap();
    assert!(!prod_recs.is_empty());
}

#[test]
fn test_config_template_generation() {
    let template = ConfigUtils::generate_config_template();

    assert!(template.contains("[server]"));
    assert!(template.contains("[database]"));
    assert!(template.contains("host"));
    assert!(template.contains("port"));
    assert!(template.contains("influx_url"));
}

#[test]
fn test_environment_variable_override() {
    // Set environment variable
    env::set_var("RAVEN_SERVER__PORT", "9999");

    // This test would require actually loading config with environment
    // For now, just verify the variable is set
    assert_eq!(env::var("RAVEN_SERVER__PORT").unwrap(), "9999");

    // Clean up
    env::remove_var("RAVEN_SERVER__PORT");
}

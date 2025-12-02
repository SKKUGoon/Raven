use config::{Config, ConfigError, File, Environment};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerConfig,
    pub tibs: TibsConfig,
    pub influx: InfluxConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port_spot: u16,
    pub port_futures: u16,
    pub port_persistence: u16,
    pub port_bars: u16,
    pub port_tibs: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TibsConfig {
    pub initial_size: f64,
    pub initial_p_buy: f64,
    pub alpha_size: f64,
    pub alpha_imbl: f64,
    pub size_min: f64,
    pub size_max: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct InfluxConfig {
    pub url: String,
    pub token: String,
    pub org: String,
    pub bucket: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "test".into());

        let s = Config::builder()
            // Start with default values from "test.toml"
            .add_source(File::with_name("test"))
            // Add in the current environment file if it exists (e.g. "prod.toml")
            // This file should be in .gitignore
            .add_source(File::with_name(&run_mode).required(false))
            // Add in a local config file usually not checked in
            .add_source(File::with_name("local").required(false))
            // Add in settings from the environment (with a prefix of APP)
            // e.g. `APP_DEBUG=1` would set the `debug` key
            .add_source(Environment::with_prefix("raven").separator("__"))
            .build()?;

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_deserialize()
    }
}


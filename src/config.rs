use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;
use std::collections::HashMap;
use std::str::FromStr;

use crate::domain::instrument::Instrument;
use crate::domain::venue::VenueId;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerConfig,
    pub tibs: TibsConfig,
    pub influx: InfluxConfig,
    pub timescale: TimescaleConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub routing: RoutingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port_binance_spot: u16,
    pub port_binance_futures: u16,
    pub port_tick_persistence: u16,
    pub port_bar_persistence: u16,
    pub port_timebar_minutes: u16,
    #[serde(default = "default_port_timebar_seconds")]
    pub port_timebar_seconds: u16,
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
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_batch_interval")]
    pub batch_interval_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TimescaleConfig {
    pub url: String,
    #[serde(default = "default_timescale_schema")]
    pub schema: String,
}

fn default_batch_size() -> usize {
    1000
}

fn default_batch_interval() -> u64 {
    1000
}

fn default_port_timebar_seconds() -> u16 {
    50053
}

fn default_timescale_schema() -> String {
    "warehouse".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RoutingConfig {
    /// Optional allowlist. If empty/omitted, Raven defaults to all known venues.
    #[serde(default)]
    pub venue_include: Vec<String>,
    /// Optional denylist.
    #[serde(default)]
    pub venue_exclude: Vec<String>,
    /// Canonical instrument -> venue -> venue-specific symbol.
    ///
    /// Example:
    /// symbol_map."PEPE/USDT"."BINANCE_FUTURES" = "1000PEPEUSDT"
    #[serde(default)]
    pub symbol_map: HashMap<String, HashMap<String, String>>,
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
            // Add in settings from the environment.
            // Example: `RAVEN__SERVER__PORT_BINANCE_SPOT=50099`
            .add_source(Environment::with_prefix("RAVEN").separator("__"))
            .build()?;

        // You can deserialize (and thus freeze) the entire configuration as
        let settings: Self = s.try_deserialize()?;
        settings.validate()?;
        Ok(settings)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        self.validate_routing()
    }

    fn validate_routing(&self) -> Result<(), ConfigError> {
        // Validate venue lists
        for v in self.routing.venue_include.iter().chain(self.routing.venue_exclude.iter()) {
            VenueId::from_str(v)
                .map_err(|e| ConfigError::Message(format!("Invalid routing venue '{v}': {e}")))?;
        }

        // Validate symbol map keys and venue keys
        for (instrument_key, venue_map) in &self.routing.symbol_map {
            Instrument::from_str(instrument_key).map_err(|e| {
                ConfigError::Message(format!(
                    "Invalid routing.symbol_map key '{instrument_key}': {e} (expected BASE/QUOTE)"
                ))
            })?;
            for (venue_key, venue_symbol) in venue_map {
                VenueId::from_str(venue_key).map_err(|e| {
                    ConfigError::Message(format!(
                        "Invalid routing.symbol_map venue '{venue_key}' for '{instrument_key}': {e}"
                    ))
                })?;
                if venue_symbol.trim().is_empty() {
                    return Err(ConfigError::Message(format!(
                        "Invalid routing.symbol_map value for '{instrument_key}'/'{venue_key}': symbol is empty"
                    )));
                }
            }
        }

        Ok(())
    }
}

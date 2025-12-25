use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::str::FromStr;

use crate::domain::instrument::Instrument;
use crate::domain::venue::VenueId;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerConfig,
    #[serde(default)]
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
    #[serde(default = "default_port_vpin")]
    pub port_vpin: u16,
    /// Legacy single TIBS service port (deprecated; use port_tibs_small/port_tibs_large).
    #[serde(default = "default_port_tibs_legacy")]
    pub port_tibs: u16,
    /// Small TIBS service port (used by `raven_tibs_small`).
    #[serde(default = "default_port_tibs_small")]
    pub port_tibs_small: u16,
    /// Large TIBS service port (used by `raven_tibs_large`).
    #[serde(default = "default_port_tibs_large")]
    pub port_tibs_large: u16,
    /// Small TRBS (tick run bars) service port (used by `raven_trbs_small`).
    #[serde(default = "default_port_trbs_small")]
    pub port_trbs_small: u16,
    /// Large TRBS (tick run bars) service port (used by `raven_trbs_large`).
    #[serde(default = "default_port_trbs_large")]
    pub port_trbs_large: u16,
    /// Small VIBS (volume imbalance bars) service port (used by `raven_vibs_small`).
    #[serde(default = "default_port_vibs_small")]
    pub port_vibs_small: u16,
    /// Large VIBS (volume imbalance bars) service port (used by `raven_vibs_large`).
    #[serde(default = "default_port_vibs_large")]
    pub port_vibs_large: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TibsConfig {
    pub initial_size: f64,
    pub initial_p_buy: f64,
    pub alpha_size: f64,
    pub alpha_imbl: f64,
    /// Absolute lower bound for expected bar size (ticks per bar).
    /// Deprecated in favor of `size_min_pct` / `size_max_pct`.
    #[serde(default)]
    pub size_min: Option<f64>,
    /// Absolute upper bound for expected bar size (ticks per bar).
    /// Deprecated in favor of `size_min_pct` / `size_max_pct`.
    #[serde(default)]
    pub size_max: Option<f64>,

    /// Percentage band (relative to `initial_size`) to clamp expected bar size.
    /// If set, bounds are:
    /// - min = initial_size * (1 - size_min_pct)
    /// - max = initial_size * (1 + size_max_pct)
    #[serde(default)]
    pub size_min_pct: Option<f64>,
    #[serde(default)]
    pub size_max_pct: Option<f64>,

    /// Optional additional named profiles, e.g. `[tibs.profiles.small]`.
    /// These run in parallel and emit candles with interval `tib_<name>` (unless the name already starts with `tib`).
    #[serde(default)]
    pub profiles: HashMap<String, TibsProfileConfig>,
}

impl Default for TibsConfig {
    fn default() -> Self {
        Self {
            initial_size: 100.0,
            initial_p_buy: 0.7,
            alpha_size: 0.1,
            alpha_imbl: 0.1,
            size_min: None,
            size_max: None,
            size_min_pct: Some(0.1),
            size_max_pct: Some(0.1),
            profiles: HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TibsProfileConfig {
    #[serde(default)]
    pub initial_size: Option<f64>,
    #[serde(default)]
    pub initial_p_buy: Option<f64>,
    #[serde(default)]
    pub alpha_size: Option<f64>,
    #[serde(default)]
    pub alpha_imbl: Option<f64>,
    #[serde(default)]
    pub size_min: Option<f64>,
    #[serde(default)]
    pub size_max: Option<f64>,
    #[serde(default)]
    pub size_min_pct: Option<f64>,
    #[serde(default)]
    pub size_max_pct: Option<f64>,
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

fn default_port_vpin() -> u16 {
    50054
}

fn default_port_tibs_small() -> u16 {
    50062
}

fn default_port_tibs_large() -> u16 {
    50052
}

fn default_port_tibs_legacy() -> u16 {
    // Historically, the single `raven_tibs` service used the same port as the current "large" profile.
    default_port_tibs_large()
}

fn default_port_trbs_small() -> u16 {
    50063
}

fn default_port_trbs_large() -> u16 {
    50064
}

fn default_port_vibs_small() -> u16 {
    50065
}

fn default_port_vibs_large() -> u16 {
    50066
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
        for v in self
            .routing
            .venue_include
            .iter()
            .chain(self.routing.venue_exclude.iter())
        {
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

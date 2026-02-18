use config::{Config, ConfigError, Environment, File, FileFormat};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
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
    #[serde(default)]
    pub binance_klines: BinanceKlinesConfig,
    #[serde(default)]
    pub deribit: DeribitConfig,
    #[serde(default)]
    pub binance_rest: BinanceRestConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port_binance_spot: u16,
    pub port_binance_futures: u16,
    #[serde(default = "default_port_binance_futures_klines")]
    pub port_binance_futures_klines: u16,
    pub port_tick_persistence: u16,
    pub port_bar_persistence: u16,
    #[serde(default = "default_port_kline_persistence")]
    pub port_kline_persistence: u16,
    pub port_timebar_minutes: u16,
    #[serde(default = "default_port_timebar_seconds")]
    pub port_timebar_seconds: u16,
    #[serde(default = "default_port_vpin")]
    pub port_vpin: u16,
    /// Small TIBS service port (used by `raven_tibs` via ServiceSpec).
    #[serde(default = "default_port_tibs_small")]
    pub port_tibs_small: u16,
    /// Large TIBS service port (used by `raven_tibs` via ServiceSpec).
    #[serde(default = "default_port_tibs_large")]
    pub port_tibs_large: u16,
    /// Small TRBS (tick run bars) service port (used by `raven_trbs` via ServiceSpec).
    #[serde(default = "default_port_trbs_small")]
    pub port_trbs_small: u16,
    /// Large TRBS (tick run bars) service port (used by `raven_trbs` via ServiceSpec).
    #[serde(default = "default_port_trbs_large")]
    pub port_trbs_large: u16,
    /// Small VIBS (volume imbalance bars) service port (used by `raven_vibs` via ServiceSpec).
    #[serde(default = "default_port_vibs_small")]
    pub port_vibs_small: u16,
    /// Large VIBS (volume imbalance bars) service port (used by `raven_vibs` via ServiceSpec).
    #[serde(default = "default_port_vibs_large")]
    pub port_vibs_large: u16,
    /// Deribit BTC options ticker service port.
    #[serde(default = "default_port_deribit_ticker")]
    pub port_deribit_ticker: u16,
    /// Deribit BTC options trades service port.
    #[serde(default = "default_port_deribit_trades")]
    pub port_deribit_trades: u16,
    /// Deribit BTC underlying price index service port.
    #[serde(default = "default_port_deribit_index")]
    pub port_deribit_index: u16,
    /// Binance Futures liquidation stream port.
    #[serde(default = "default_port_binance_futures_liquidations")]
    pub port_binance_futures_liquidations: u16,
    /// Binance Futures funding rate poller port.
    #[serde(default = "default_port_binance_futures_funding")]
    pub port_binance_futures_funding: u16,
    /// Binance Futures open interest poller port.
    #[serde(default = "default_port_binance_futures_oi")]
    pub port_binance_futures_oi: u16,
    /// Binance Options ticker poller port (OI + IV combined).
    #[serde(default = "default_port_binance_options")]
    pub port_binance_options: u16,
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

#[derive(Debug, Deserialize, Clone)]
pub struct BinanceKlinesConfig {
    /// Futures kline websocket base URL (expects JSON SUBSCRIBE messages).
    #[serde(default = "default_binance_futures_ws_url")]
    pub ws_url: String,
    /// Kline interval (e.g. "1m", "5m", "1h").
    #[serde(default = "default_binance_kline_interval")]
    pub interval: String,
    /// Number of symbols per connection (shard size).
    #[serde(default = "default_binance_kline_shard_size")]
    pub shard_size: usize,
    /// Number of websocket connections (shards) to spawn.
    #[serde(default = "default_binance_kline_connections")]
    pub connections: usize,
    /// Binance hard limit is 1024 streams per connection.
    #[serde(default = "default_binance_kline_max_streams")]
    pub max_streams_per_connection: usize,
    /// Broadcast channel capacity for outgoing gRPC subscribers.
    #[serde(default = "default_binance_kline_channel_capacity")]
    pub channel_capacity: usize,
    /// Optional explicit list of venue symbols to subscribe to (e.g. ["BTCUSDT","ETHUSDT"]).
    /// If omitted/empty, Raven falls back to futures entries in `routing.symbol_map`.
    #[serde(default)]
    pub symbols: Vec<String>,
}

impl Default for BinanceKlinesConfig {
    fn default() -> Self {
        Self {
            ws_url: default_binance_futures_ws_url(),
            interval: default_binance_kline_interval(),
            shard_size: default_binance_kline_shard_size(),
            connections: default_binance_kline_connections(),
            max_streams_per_connection: default_binance_kline_max_streams(),
            channel_capacity: default_binance_kline_channel_capacity(),
            symbols: Vec::new(),
        }
    }
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

fn default_port_binance_futures_klines() -> u16 {
    50003
}

fn default_port_kline_persistence() -> u16 {
    50093
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

fn default_binance_futures_ws_url() -> String {
    "wss://fstream.binance.com/ws".to_string()
}

fn default_binance_kline_interval() -> String {
    "1m".to_string()
}

fn default_binance_kline_shard_size() -> usize {
    50
}

fn default_binance_kline_connections() -> usize {
    10
}

fn default_binance_kline_max_streams() -> usize {
    1024
}

fn default_binance_kline_channel_capacity() -> usize {
    10_000
}

#[derive(Debug, Deserialize, Clone)]
pub struct DeribitConfig {
    /// WebSocket URL (production or test).
    #[serde(default = "default_deribit_ws_url")]
    pub ws_url: String,
    /// REST API base for public/get_instruments (no trailing slash).
    #[serde(default = "default_deribit_rest_url")]
    pub rest_url: String,
    /// Broadcast channel capacity for outgoing gRPC subscribers.
    #[serde(default = "default_deribit_channel_capacity")]
    pub channel_capacity: usize,
}

fn default_port_binance_futures_liquidations() -> u16 {
    50004
}

fn default_port_binance_futures_funding() -> u16 {
    50005
}

fn default_port_binance_futures_oi() -> u16 {
    50006
}

fn default_port_binance_options() -> u16 {
    50007
}

fn default_port_deribit_ticker() -> u16 {
    50094
}

fn default_port_deribit_trades() -> u16 {
    50095
}

fn default_port_deribit_index() -> u16 {
    50096
}

fn default_deribit_ws_url() -> String {
    "wss://www.deribit.com/ws/api/v2".to_string()
}

fn default_deribit_rest_url() -> String {
    "https://www.deribit.com/api/v2".to_string()
}

fn default_deribit_channel_capacity() -> usize {
    10_000
}

#[derive(Debug, Deserialize, Clone)]
pub struct BinanceRestConfig {
    /// Binance Futures REST base URL.
    #[serde(default = "default_binance_futures_rest_url")]
    pub futures_rest_url: String,
    /// Binance European Options REST base URL.
    #[serde(default = "default_binance_options_rest_url")]
    pub options_rest_url: String,
    /// Polling interval for funding rate (seconds).
    #[serde(default = "default_funding_poll_secs")]
    pub funding_poll_secs: u64,
    /// Polling interval for open interest (seconds).
    #[serde(default = "default_oi_poll_secs")]
    pub oi_poll_secs: u64,
    /// Polling interval for Binance options ticker (seconds).
    #[serde(default = "default_options_poll_secs")]
    pub options_poll_secs: u64,
    /// Symbols to poll for funding rate and OI (e.g. ["BTCUSDT"]).
    #[serde(default = "default_rest_symbols")]
    pub symbols: Vec<String>,
    /// Broadcast channel capacity.
    #[serde(default = "default_binance_rest_channel_capacity")]
    pub channel_capacity: usize,
}

fn default_binance_futures_rest_url() -> String {
    "https://fapi.binance.com".to_string()
}

fn default_binance_options_rest_url() -> String {
    "https://eapi.binance.com".to_string()
}

fn default_funding_poll_secs() -> u64 {
    60
}

fn default_oi_poll_secs() -> u64 {
    5
}

fn default_options_poll_secs() -> u64 {
    10
}

fn default_rest_symbols() -> Vec<String> {
    vec!["BTCUSDT".to_string()]
}

fn default_binance_rest_channel_capacity() -> usize {
    10_000
}

impl Default for BinanceRestConfig {
    fn default() -> Self {
        Self {
            futures_rest_url: default_binance_futures_rest_url(),
            options_rest_url: default_binance_options_rest_url(),
            funding_poll_secs: default_funding_poll_secs(),
            oi_poll_secs: default_oi_poll_secs(),
            options_poll_secs: default_options_poll_secs(),
            symbols: default_rest_symbols(),
            channel_capacity: default_binance_rest_channel_capacity(),
        }
    }
}

impl Default for DeribitConfig {
    fn default() -> Self {
        Self {
            ws_url: default_deribit_ws_url(),
            rest_url: default_deribit_rest_url(),
            channel_capacity: default_deribit_channel_capacity(),
        }
    }
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

        // Embed `test.toml` defaults so binaries don't require `./test.toml` at runtime.
        //
        // This makes `ravenctl` usable when installed system-wide (e.g. /usr/local/bin) and run
        // from arbitrary directories.
        const DEFAULT_TEST_TOML: &str = include_str!("../test.toml");

        let mut builder =
            Config::builder().add_source(File::from_str(DEFAULT_TEST_TOML, FileFormat::Toml));

        // Highest priority file override: explicit path.
        //
        // This is intended to be set by `ravenctl setup` (persisted) or by the user directly.
        if let Ok(path) = env::var("RAVEN_CONFIG_FILE") {
            builder = builder.add_source(File::from(PathBuf::from(path)).required(true));
        } else {
            // Backward compatibility: in repo/dev workflows, allow a local `./test.toml` to
            // override the compiled-in defaults (best-effort).
            builder = builder.add_source(File::with_name("test").required(false));

            // 1) CWD: `${RUN_MODE}.toml` (e.g. ./prod.toml)
            builder = builder.add_source(File::with_name(&run_mode).required(false));

            // 2) Standard locations (best-effort)
            builder = builder.add_source(
                File::from(PathBuf::from(format!("/etc/raven/{run_mode}.toml"))).required(false),
            );
            if let Ok(home) = env::var("HOME") {
                builder = builder
                    .add_source(
                        File::from(
                            PathBuf::from(&home)
                                .join(".config/raven")
                                .join(format!("{run_mode}.toml")),
                        )
                        .required(false),
                    )
                    .add_source(
                        File::from(
                            PathBuf::from(&home)
                                .join(".raven")
                                .join(format!("{run_mode}.toml")),
                        )
                        .required(false),
                    );
            }
        }

        // Local overrides (optional).
        if let Ok(path) = env::var("RAVEN_LOCAL_CONFIG_FILE") {
            builder = builder.add_source(File::from(PathBuf::from(path)).required(false));
        } else {
            builder = builder.add_source(File::with_name("local").required(false));
        }

        // Add in settings from the environment.
        // Example: `RAVEN__SERVER__PORT_BINANCE_SPOT=50099`
        let s = builder
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

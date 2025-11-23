use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod config;
pub mod db;
pub mod error;

pub use config::{ConfigLoader, ConfigUtils, RuntimeConfig};
pub use db::{
    CircuitBreaker, CircuitBreakerConfig, DeadLetterEntry, DeadLetterQueue, EnhancedInfluxClient,
    InfluxClient,
};
pub use error::{RavenError, RavenResult};

/// Return the current UTC timestamp in milliseconds since the Unix epoch.
/// Falls back to zero if the system clock is earlier than the epoch.
pub fn current_timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as i64
}

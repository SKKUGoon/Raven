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

pub use crate::{raven_bail, raven_error};

/// Return the current UTC timestamp in milliseconds since the Unix epoch.
/// Falls back to zero if the system clock is earlier than the epoch.
pub fn current_timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as i64
}

/// Checks if a TCP port is available for binding on the given host.
pub async fn check_port_availability(host: &str, port: u16, name: &str) -> RavenResult<()> {
    let bind_addr = format!("{host}:{port}");
    match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(_) => {
            tracing::info!("  * {name} port {port} is available");
            Ok(())
        }
        Err(e) => {
            tracing::error!("  * Cannot bind to {name} port {port}: {e}");
            raven_bail!(raven_error!(
                configuration,
                format!("  * {name} port {port} is not available: {e}")
            ));
        }
    }
}

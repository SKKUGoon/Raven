// Project Raven - Core Library
// "The ravens are the memory of the realm"

pub mod app;
pub mod citadel;
pub mod client_manager;
pub mod config;
pub mod control;
pub mod data_handlers;
pub mod database;
pub mod error;
pub mod exchanges;
pub mod logging;
pub mod monitoring;
pub mod server;
pub mod subscription_manager;
pub mod time;

// Generated protobuf modules
pub mod proto {
    tonic::include_proto!("raven");
}

// Re-export commonly used types
pub use config::{ConfigLoader, ConfigManager, ConfigUtils, RuntimeConfig, RuntimeConfigBuilder};
pub use error::{RavenError, RavenResult};
pub use logging::LoggingConfig;
pub use proto::*;
pub use time::current_timestamp_millis;

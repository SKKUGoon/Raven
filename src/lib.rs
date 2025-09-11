// Project Raven - Core Library
// "The ravens are the memory of the realm"

pub mod circuit_breaker;
pub mod citadel;
pub mod client_manager;
pub mod config;
pub mod data_handlers;
pub mod database;
pub mod dead_letter_queue;
pub mod error;
pub mod logging;
pub mod monitoring;
pub mod server;
pub mod snapshot_service;
pub mod subscription_manager;
pub mod types;

// Generated protobuf modules
pub mod proto {
    tonic::include_proto!("raven");
}

// Re-export commonly used types
pub use config::Config;
pub use error::{RavenError, RavenResult};
pub use logging::LoggingConfig;
pub use proto::*;

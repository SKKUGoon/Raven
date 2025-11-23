// Project Raven - Core Library

pub mod common;
pub mod controller;
pub mod server;

// Generated protobuf modules
pub mod proto {
    tonic::include_proto!("raven");
}

// Re-export commonly used types
pub use common::{
    current_timestamp_millis, ConfigLoader, ConfigUtils, LoggingConfig, RavenError, RavenResult,
    RuntimeConfig,
};
pub use proto::*;

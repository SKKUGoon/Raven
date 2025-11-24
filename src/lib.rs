// Project Raven - Core Library

pub mod clients;
pub mod common;
pub mod controller;
pub mod server;

// Generated protobuf modules
pub mod proto {
    tonic::include_proto!("raven");
}

// Re-export commonly used types
pub use common::{
    current_timestamp_millis, ConfigLoader, ConfigUtils, RavenError, RavenResult, RuntimeConfig,
};
pub use proto::*;

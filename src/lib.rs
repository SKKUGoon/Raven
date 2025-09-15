// Project Raven - Core Library
// "The ravens are the memory of the realm"

pub mod citadel;
pub mod client_manager;
pub mod config;
pub mod data_handlers;
pub mod database;
pub mod error;
pub mod exchanges;
pub mod logging;
pub mod monitoring;
pub mod server;
pub mod subscription_manager;

// Legacy re-exports for backward compatibility
pub mod types {
    pub use crate::citadel::storage::*;
}
pub mod snapshot_service {
    pub use crate::citadel::streaming::*;
}
pub mod circuit_breaker {
    pub use crate::database::circuit_breaker::*;
}
pub mod dead_letter_queue {
    pub use crate::database::dead_letter_queue::*;
}

// Generated protobuf modules
pub mod proto {
    tonic::include_proto!("raven");
}

// Re-export commonly used types
pub use config::Config;
pub use error::{RavenError, RavenResult};
pub use logging::LoggingConfig;
pub use proto::*;

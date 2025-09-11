// Private Data Handler Module
// "Encrypted ravens carrying secrets across the realm with utmost discretion"

mod config;
mod handler;
mod metrics;
mod storage;
mod types;

pub use config::PrivateDataConfig;
pub use handler::PrivateDataHandler;
pub use metrics::PrivateDataMetrics;
pub use storage::PrivateDataStorage;
pub use types::{
    AccessLevel, BalanceData, ClientPermissions, PositionUpdateData, PrivateData,
    SecureChannelMessage, WalletUpdateData,
};

#[cfg(test)]
mod tests;

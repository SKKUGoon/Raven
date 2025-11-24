use serde::{Deserialize, Serialize};

/// Configuration for the Data Engine
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataEngineConfig {
    // Config fields removed as we trust the exchange
    // Empty config for
    // 1. Future proofing: If we need to add a config (max threads or etc.) later
    // 2. Serialization.
}

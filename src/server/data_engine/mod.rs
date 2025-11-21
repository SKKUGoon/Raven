// Data Engine - Unified Data Management System
// "Guards the integrity of our data, stores it atomically, and streams it to the realm"

// Sub-modules
pub mod config;
pub mod engine;
pub mod ingestion; // Ingestion orchestration for data collectors
pub mod metrics;
pub mod storage; // Atomic data storage (formerly types)
pub mod streaming; // Data streaming to clients (formerly snapshot_service)
pub mod validation;

// Re-export core types and functionality
pub use config::DataEngineConfig;
pub use engine::DataEngine;
pub use metrics::DataEngineMetrics;
pub use storage::{
    atomic::{AtomicOrderBook, AtomicTrade, HighFrequencyStorage},
    snapshots::{OrderBookSnapshot, TradeSnapshot},
    {CandleData, OrderBookData, TickerData, TradeData, TradeSide},
};
pub use streaming::{SnapshotBatch, SnapshotConfig, SnapshotMetrics, SnapshotService};
pub use validation::ValidationRules;

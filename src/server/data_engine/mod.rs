// Sub-modules
pub mod config;
pub mod engine;
pub mod ingestion; // Ingestion orchestration for data collectors
pub mod metrics;
pub mod storage; // Atomic data storage (formerly types)

// Re-export core types and functionality
pub use config::DataEngineConfig;
pub use engine::DataEngine;
pub use metrics::DataEngineMetrics;
pub use storage::{
    atomic::{AtomicOrderBook, AtomicTrade, HighFrequencyStorage},
    snapshots::{OrderBookSnapshot, TradeSnapshot},
    {CandleData, OrderBookData, TickerData, TradeData, TradeSide},
};

// Core Types and Data Structures
// "The fundamental building blocks of the realm"

use serde::{Deserialize, Serialize};

// Re-export submodules
pub mod atomic;
pub mod snapshots;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use atomic::{AtomicOrderBook, AtomicTrade, HighFrequencyStorage};
pub use snapshots::{OrderBookSnapshot, TradeSnapshot};

// Constants for price/quantity conversion
pub const PRICE_SCALE: f64 = 100_000_000.0; // 8 decimal places precision
pub const QUANTITY_SCALE: f64 = 100_000_000.0; // 8 decimal places precision

// WebSocket data interface structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookData {
    pub symbol: String,
    pub timestamp: i64,        // Unix timestamp in milliseconds
    pub bids: Vec<(f64, f64)>, // (price, quantity)
    pub asks: Vec<(f64, f64)>, // (price, quantity)
    pub sequence: u64,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub symbol: String,
    pub timestamp: i64, // Unix timestamp in milliseconds
    pub price: f64,
    pub quantity: f64,
    pub side: String, // "buy" or "sell"
    pub trade_id: String,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandleData {
    pub symbol: String,
    pub timestamp: i64, // Unix timestamp in milliseconds (candle open time)
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub interval: String, // "1m", "5m", "1h", etc.
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRateData {
    pub symbol: String,
    pub timestamp: i64, // Unix timestamp in milliseconds
    pub rate: f64,
    pub next_funding_time: i64, // Unix timestamp in milliseconds
    pub exchange: String,
}

// Utility functions for price/quantity conversion
pub fn price_to_atomic(price: f64) -> u64 {
    (price * PRICE_SCALE) as u64
}

pub fn atomic_to_price(atomic_price: u64) -> f64 {
    atomic_price as f64 / PRICE_SCALE
}

pub fn quantity_to_atomic(quantity: f64) -> u64 {
    (quantity * QUANTITY_SCALE) as u64
}

pub fn atomic_to_quantity(atomic_quantity: u64) -> f64 {
    atomic_quantity as f64 / QUANTITY_SCALE
}

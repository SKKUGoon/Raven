// Core Types and Data Structures
// "The fundamental building blocks of the realm"

use crate::exchanges::types::Exchange;
use serde::{Deserialize, Serialize};

// Re-export submodules
pub mod atomic;
pub mod snapshots;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use atomic::{AtomicOrderBook, AtomicTrade, HighFrequencyStorage};
pub use snapshots::{OrderBookLevel, OrderBookSnapshot, TradeSnapshot};

// Constants for price/quantity conversion
pub const PRICE_SCALE: f64 = 100_000_000.0; // 8 decimal places precision
pub const QUANTITY_SCALE: f64 = 100_000_000.0; // 8 decimal places precision

// Core market data structures - unified between WebSocket and storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookData {
    pub symbol: String,
    pub timestamp: i64,        // Unix timestamp in milliseconds
    pub bids: Vec<(f64, f64)>, // (price, quantity)
    pub asks: Vec<(f64, f64)>, // (price, quantity)
    pub sequence: u64,
    pub exchange: Exchange, // Use Exchange enum for consistency
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub symbol: String,
    pub timestamp: i64, // Unix timestamp in milliseconds
    pub price: f64,
    pub quantity: f64,
    pub side: TradeSide, // Use enum instead of string
    pub trade_id: String,
    pub exchange: Exchange, // Use Exchange enum for consistency
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
    pub interval: String,   // "1m", "5m", "1h", etc.
    pub exchange: Exchange, // Use Exchange enum for consistency
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerData {
    pub symbol: String,
    pub timestamp: i64,
    pub price: f64,
    pub weighted_average_price: f64,
    pub exchange: Exchange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRateData {
    pub symbol: String,
    pub timestamp: i64, // Unix timestamp in milliseconds
    pub rate: f64,
    pub next_funding_time: i64, // Unix timestamp in milliseconds
    pub exchange: Exchange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

impl TradeSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            TradeSide::Buy => "buy",
            TradeSide::Sell => "sell",
        }
    }
}

impl std::str::FromStr for TradeSide {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "buy" => Ok(TradeSide::Buy),
            "sell" => Ok(TradeSide::Sell),
            _ => Ok(TradeSide::Buy), // Default to buy
        }
    }
}

impl std::fmt::Display for TradeSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl PartialEq<&str> for TradeSide {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
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

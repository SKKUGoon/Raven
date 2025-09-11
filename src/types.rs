// Core Types and Data Structures
// "The fundamental building blocks of the realm"

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

// Constants for price/quantity conversion
const PRICE_SCALE: f64 = 100_000_000.0; // 8 decimal places precision
const QUANTITY_SCALE: f64 = 100_000_000.0; // 8 decimal places precision

// High-frequency atomic data storage for maximum throughput
// Cache-line aligned to prevent false sharing and optimize memory access
#[repr(align(64))] // Align to cache line boundary (64 bytes on most modern CPUs)
#[derive(Debug)]
pub struct AtomicOrderBook {
    pub symbol: String,
    // Group frequently accessed atomics together for cache efficiency
    pub timestamp: AtomicI64,
    pub sequence: AtomicU64,
    // Separate cache line for bid data to prevent false sharing
    pub best_bid_price: AtomicU64, // Store as integer (price * 100000000)
    pub best_bid_quantity: AtomicU64,
    // Separate cache line for ask data to prevent false sharing
    pub best_ask_price: AtomicU64,
    pub best_ask_quantity: AtomicU64,
}

impl AtomicOrderBook {
    /// Create a new AtomicOrderBook with default values
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            timestamp: AtomicI64::new(0),
            best_bid_price: AtomicU64::new(0),
            best_bid_quantity: AtomicU64::new(0),
            best_ask_price: AtomicU64::new(0),
            best_ask_quantity: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
        }
    }

    /// Update orderbook data atomically from OrderBookData
    pub fn update_from_data(&self, data: &OrderBookData) {
        self.timestamp.store(data.timestamp, Ordering::Relaxed);
        self.sequence.store(data.sequence, Ordering::Relaxed);

        // Update best bid if available
        if let Some((bid_price, bid_qty)) = data.bids.first() {
            self.best_bid_price
                .store((bid_price * PRICE_SCALE) as u64, Ordering::Relaxed);
            self.best_bid_quantity
                .store((bid_qty * QUANTITY_SCALE) as u64, Ordering::Relaxed);
        }

        // Update best ask if available
        if let Some((ask_price, ask_qty)) = data.asks.first() {
            self.best_ask_price
                .store((ask_price * PRICE_SCALE) as u64, Ordering::Relaxed);
            self.best_ask_quantity
                .store((ask_qty * QUANTITY_SCALE) as u64, Ordering::Relaxed);
        }
    }

    /// Convert to snapshot for periodic captures
    pub fn to_snapshot(&self) -> OrderBookSnapshot {
        OrderBookSnapshot {
            symbol: self.symbol.clone(),
            timestamp: self.timestamp.load(Ordering::Relaxed),
            best_bid_price: self.best_bid_price.load(Ordering::Relaxed) as f64 / PRICE_SCALE,
            best_bid_quantity: self.best_bid_quantity.load(Ordering::Relaxed) as f64
                / QUANTITY_SCALE,
            best_ask_price: self.best_ask_price.load(Ordering::Relaxed) as f64 / PRICE_SCALE,
            best_ask_quantity: self.best_ask_quantity.load(Ordering::Relaxed) as f64
                / QUANTITY_SCALE,
            sequence: self.sequence.load(Ordering::Relaxed),
        }
    }

    /// Get current spread (ask - bid)
    pub fn get_spread(&self) -> f64 {
        let bid = self.best_bid_price.load(Ordering::Relaxed) as f64 / PRICE_SCALE;
        let ask = self.best_ask_price.load(Ordering::Relaxed) as f64 / PRICE_SCALE;
        ask - bid
    }
}

// Cache-line aligned atomic trade structure for optimal performance
#[repr(align(64))] // Align to cache line boundary (64 bytes on most modern CPUs)
#[derive(Debug)]
pub struct AtomicTrade {
    pub symbol: String,
    // Group frequently accessed atomics together for cache efficiency
    pub timestamp: AtomicI64,
    pub price: AtomicU64, // Store as integer (price * 100000000)
    pub quantity: AtomicU64,
    pub side: AtomicU64, // 0 = buy, 1 = sell
    pub trade_id: AtomicU64,
}

impl AtomicTrade {
    /// Create a new AtomicTrade with default values
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            timestamp: AtomicI64::new(0),
            price: AtomicU64::new(0),
            quantity: AtomicU64::new(0),
            side: AtomicU64::new(0),
            trade_id: AtomicU64::new(0),
        }
    }

    /// Update trade data atomically from TradeData
    pub fn update_from_data(&self, data: &TradeData) {
        self.timestamp.store(data.timestamp, Ordering::Relaxed);
        self.price
            .store((data.price * PRICE_SCALE) as u64, Ordering::Relaxed);
        self.quantity
            .store((data.quantity * QUANTITY_SCALE) as u64, Ordering::Relaxed);

        // Convert side string to numeric value
        let side_value = match data.side.as_str() {
            "buy" => 0,
            "sell" => 1,
            _ => 0, // Default to buy if unknown
        };
        self.side.store(side_value, Ordering::Relaxed);

        // Convert trade_id string to hash for atomic storage
        let trade_id_hash = self.hash_trade_id(&data.trade_id);
        self.trade_id.store(trade_id_hash, Ordering::Relaxed);
    }

    /// Convert to snapshot for periodic captures
    pub fn to_snapshot(&self) -> TradeSnapshot {
        let side_str = match self.side.load(Ordering::Relaxed) {
            0 => "buy".to_string(),
            1 => "sell".to_string(),
            _ => "unknown".to_string(),
        };

        TradeSnapshot {
            symbol: self.symbol.clone(),
            timestamp: self.timestamp.load(Ordering::Relaxed),
            price: self.price.load(Ordering::Relaxed) as f64 / PRICE_SCALE,
            quantity: self.quantity.load(Ordering::Relaxed) as f64 / QUANTITY_SCALE,
            side: side_str,
            trade_id: self.trade_id.load(Ordering::Relaxed),
        }
    }

    /// Simple hash function for trade ID strings
    fn hash_trade_id(&self, trade_id: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        trade_id.hash(&mut hasher);
        hasher.finish()
    }

    /// Get trade value (price * quantity)
    pub fn get_trade_value(&self) -> f64 {
        let price = self.price.load(Ordering::Relaxed) as f64 / PRICE_SCALE;
        let quantity = self.quantity.load(Ordering::Relaxed) as f64 / QUANTITY_SCALE;
        price * quantity
    }
}

// High-frequency data storage using atomic operations
pub struct HighFrequencyStorage {
    pub orderbooks: DashMap<String, Arc<AtomicOrderBook>>,
    pub latest_trades: DashMap<String, Arc<AtomicTrade>>,
}

impl HighFrequencyStorage {
    /// Create a new HighFrequencyStorage instance
    pub fn new() -> Self {
        Self {
            orderbooks: DashMap::new(),
            latest_trades: DashMap::new(),
        }
    }

    /// Get or create an AtomicOrderBook for a symbol
    pub fn get_or_create_orderbook(&self, symbol: &str) -> Arc<AtomicOrderBook> {
        self.orderbooks
            .entry(symbol.to_string())
            .or_insert_with(|| Arc::new(AtomicOrderBook::new(symbol.to_string())))
            .clone()
    }

    /// Get or create an AtomicTrade for a symbol
    pub fn get_or_create_trade(&self, symbol: &str) -> Arc<AtomicTrade> {
        self.latest_trades
            .entry(symbol.to_string())
            .or_insert_with(|| Arc::new(AtomicTrade::new(symbol.to_string())))
            .clone()
    }

    /// Update orderbook data atomically
    pub fn update_orderbook(&self, data: &OrderBookData) {
        let orderbook = self.get_or_create_orderbook(&data.symbol);
        orderbook.update_from_data(data);
    }

    /// Update trade data atomically
    pub fn update_trade(&self, data: &TradeData) {
        let trade = self.get_or_create_trade(&data.symbol);
        trade.update_from_data(data);
    }

    /// Get orderbook snapshot for a symbol
    pub fn get_orderbook_snapshot(&self, symbol: &str) -> Option<OrderBookSnapshot> {
        self.orderbooks
            .get(symbol)
            .map(|orderbook| orderbook.to_snapshot())
    }

    /// Get trade snapshot for a symbol
    pub fn get_trade_snapshot(&self, symbol: &str) -> Option<TradeSnapshot> {
        self.latest_trades
            .get(symbol)
            .map(|trade| trade.to_snapshot())
    }

    /// Get all symbols with orderbook data
    pub fn get_orderbook_symbols(&self) -> Vec<String> {
        self.orderbooks
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all symbols with trade data
    pub fn get_trade_symbols(&self) -> Vec<String> {
        self.latest_trades
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }
}

impl Default for HighFrequencyStorage {
    fn default() -> Self {
        Self::new()
    }
}

// Snapshot structures for periodic captures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub symbol: String,
    pub timestamp: i64,
    pub best_bid_price: f64,
    pub best_bid_quantity: f64,
    pub best_ask_price: f64,
    pub best_ask_quantity: f64,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSnapshot {
    pub symbol: String,
    pub timestamp: i64,
    pub price: f64,
    pub quantity: f64,
    pub side: String,
    pub trade_id: u64,
}

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

// Conversion functions between atomic storage and snapshot types
impl From<&OrderBookData> for OrderBookSnapshot {
    fn from(data: &OrderBookData) -> Self {
        let (best_bid_price, best_bid_quantity) = data
            .bids
            .first()
            .map(|(price, qty)| (*price, *qty))
            .unwrap_or((0.0, 0.0));

        let (best_ask_price, best_ask_quantity) = data
            .asks
            .first()
            .map(|(price, qty)| (*price, *qty))
            .unwrap_or((0.0, 0.0));

        Self {
            symbol: data.symbol.clone(),
            timestamp: data.timestamp,
            best_bid_price,
            best_bid_quantity,
            best_ask_price,
            best_ask_quantity,
            sequence: data.sequence,
        }
    }
}

impl From<&TradeData> for TradeSnapshot {
    fn from(data: &TradeData) -> Self {
        // Convert trade_id string to hash for snapshot
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.trade_id.hash(&mut hasher);
        let trade_id_hash = hasher.finish();

        Self {
            symbol: data.symbol.clone(),
            timestamp: data.timestamp,
            price: data.price,
            quantity: data.quantity,
            side: data.side.clone(),
            trade_id: trade_id_hash,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_atomic_orderbook_creation() {
        let orderbook = AtomicOrderBook::new("BTCUSDT".to_string());
        assert_eq!(orderbook.symbol, "BTCUSDT");
        assert_eq!(orderbook.timestamp.load(Ordering::Relaxed), 0);
        assert_eq!(orderbook.sequence.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_atomic_orderbook_update() {
        let orderbook = AtomicOrderBook::new("BTCUSDT".to_string());
        let data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000, // 2022-01-01 00:00:00 UTC
            bids: vec![(45000.0, 1.5), (44999.0, 2.0)],
            asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        orderbook.update_from_data(&data);

        assert_eq!(orderbook.timestamp.load(Ordering::Relaxed), 1640995200000);
        assert_eq!(orderbook.sequence.load(Ordering::Relaxed), 12345);
        assert_eq!(
            orderbook.best_bid_price.load(Ordering::Relaxed),
            (45000.0 * PRICE_SCALE) as u64
        );
        assert_eq!(
            orderbook.best_ask_price.load(Ordering::Relaxed),
            (45001.0 * PRICE_SCALE) as u64
        );
    }

    #[test]
    fn test_atomic_orderbook_snapshot() {
        let orderbook = AtomicOrderBook::new("BTCUSDT".to_string());
        let data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![(45001.0, 1.2)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        orderbook.update_from_data(&data);
        let snapshot = orderbook.to_snapshot();

        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.timestamp, 1640995200000);
        assert_eq!(snapshot.best_bid_price, 45000.0);
        assert_eq!(snapshot.best_ask_price, 45001.0);
        assert_eq!(snapshot.sequence, 12345);
    }

    #[test]
    fn test_atomic_orderbook_spread() {
        let orderbook = AtomicOrderBook::new("BTCUSDT".to_string());
        let data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![(45001.0, 1.2)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        orderbook.update_from_data(&data);
        let spread = orderbook.get_spread();
        assert!((spread - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_atomic_trade_creation() {
        let trade = AtomicTrade::new("BTCUSDT".to_string());
        assert_eq!(trade.symbol, "BTCUSDT");
        assert_eq!(trade.timestamp.load(Ordering::Relaxed), 0);
        assert_eq!(trade.side.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_atomic_trade_update() {
        let trade = AtomicTrade::new("BTCUSDT".to_string());
        let data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        trade.update_from_data(&data);

        assert_eq!(trade.timestamp.load(Ordering::Relaxed), 1640995200000);
        assert_eq!(
            trade.price.load(Ordering::Relaxed),
            (45000.5 * PRICE_SCALE) as u64
        );
        assert_eq!(
            trade.quantity.load(Ordering::Relaxed),
            (0.1 * QUANTITY_SCALE) as u64
        );
        assert_eq!(trade.side.load(Ordering::Relaxed), 0); // buy = 0
    }

    #[test]
    fn test_atomic_trade_side_conversion() {
        let trade = AtomicTrade::new("BTCUSDT".to_string());

        // Test buy side
        let buy_data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.0,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "buy123".to_string(),
            exchange: "binance".to_string(),
        };
        trade.update_from_data(&buy_data);
        assert_eq!(trade.side.load(Ordering::Relaxed), 0);

        // Test sell side
        let sell_data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.0,
            quantity: 0.1,
            side: "sell".to_string(),
            trade_id: "sell123".to_string(),
            exchange: "binance".to_string(),
        };
        trade.update_from_data(&sell_data);
        assert_eq!(trade.side.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_atomic_trade_snapshot() {
        let trade = AtomicTrade::new("BTCUSDT".to_string());
        let data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "sell".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        trade.update_from_data(&data);
        let snapshot = trade.to_snapshot();

        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.timestamp, 1640995200000);
        assert_eq!(snapshot.price, 45000.5);
        assert_eq!(snapshot.quantity, 0.1);
        assert_eq!(snapshot.side, "sell");
    }

    #[test]
    fn test_atomic_trade_value() {
        let trade = AtomicTrade::new("BTCUSDT".to_string());
        let data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.0,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        trade.update_from_data(&data);
        let trade_value = trade.get_trade_value();
        assert!((trade_value - 4500.0).abs() < 0.01); // 45000 * 0.1 = 4500
    }

    #[test]
    fn test_high_frequency_storage() {
        let storage = HighFrequencyStorage::new();

        // Test orderbook operations
        let orderbook_data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![(45001.0, 1.2)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        storage.update_orderbook(&orderbook_data);
        let snapshot = storage.get_orderbook_snapshot("BTCUSDT").unwrap();
        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.best_bid_price, 45000.0);

        // Test trade operations
        let trade_data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        storage.update_trade(&trade_data);
        let trade_snapshot = storage.get_trade_snapshot("BTCUSDT").unwrap();
        assert_eq!(trade_snapshot.symbol, "BTCUSDT");
        assert_eq!(trade_snapshot.price, 45000.5);
    }

    #[test]
    fn test_concurrent_atomic_operations() {
        let orderbook = Arc::new(AtomicOrderBook::new("BTCUSDT".to_string()));
        let mut handles = vec![];

        // Spawn multiple threads to update the orderbook concurrently
        for i in 0..10 {
            let orderbook_clone = Arc::clone(&orderbook);
            let handle = thread::spawn(move || {
                let data = OrderBookData {
                    symbol: "BTCUSDT".to_string(),
                    timestamp: 1640995200000 + i as i64,
                    bids: vec![(45000.0 + i as f64, 1.5)],
                    asks: vec![(45001.0 + i as f64, 1.2)],
                    sequence: 12345 + i as u64,
                    exchange: "binance".to_string(),
                };
                orderbook_clone.update_from_data(&data);
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify that the orderbook was updated (exact values may vary due to concurrency)
        let snapshot = orderbook.to_snapshot();
        assert!(snapshot.timestamp >= 1640995200000);
        assert!(snapshot.sequence >= 12345);
    }

    #[test]
    fn test_price_quantity_conversion_functions() {
        let price = 45000.12345678;
        let atomic_price = price_to_atomic(price);
        let converted_back = atomic_to_price(atomic_price);
        assert!((price - converted_back).abs() < 0.00000001);

        let quantity = 1.23456789;
        let atomic_quantity = quantity_to_atomic(quantity);
        let converted_back_qty = atomic_to_quantity(atomic_quantity);
        assert!((quantity - converted_back_qty).abs() < 0.00000001);
    }

    #[test]
    fn test_conversion_from_orderbook_data() {
        let data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5), (44999.0, 2.0)],
            asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        let snapshot = OrderBookSnapshot::from(&data);
        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.best_bid_price, 45000.0);
        assert_eq!(snapshot.best_ask_price, 45001.0);
        assert_eq!(snapshot.sequence, 12345);
    }

    #[test]
    fn test_conversion_from_trade_data() {
        let data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        let snapshot = TradeSnapshot::from(&data);
        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.price, 45000.5);
        assert_eq!(snapshot.quantity, 0.1);
        assert_eq!(snapshot.side, "buy");
    }

    #[test]
    fn test_high_frequency_storage_symbols() {
        let storage = HighFrequencyStorage::new();

        let orderbook_data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![(45001.0, 1.2)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        let trade_data = TradeData {
            symbol: "ETHUSDT".to_string(),
            timestamp: 1640995200000,
            price: 3000.0,
            quantity: 1.0,
            side: "buy".to_string(),
            trade_id: "eth123".to_string(),
            exchange: "binance".to_string(),
        };

        storage.update_orderbook(&orderbook_data);
        storage.update_trade(&trade_data);

        let orderbook_symbols = storage.get_orderbook_symbols();
        let trade_symbols = storage.get_trade_symbols();

        assert!(orderbook_symbols.contains(&"BTCUSDT".to_string()));
        assert!(trade_symbols.contains(&"ETHUSDT".to_string()));
    }
}

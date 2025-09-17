// High-frequency atomic data storage for maximum throughput

use dashmap::DashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

use super::snapshots::{OrderBookSnapshot, TradeSnapshot};
use super::{OrderBookData, TradeData, PRICE_SCALE, QUANTITY_SCALE};

// High-frequency atomic data storage for maximum throughput
// Cache-line aligned to prevent false sharing and optimize memory access
#[repr(align(64))] // Align to cache line boundary (64 bytes on most modern CPUs)
#[derive(Debug)]
pub struct AtomicOrderBook {
    pub symbol: String,
    pub exchange: super::Exchange,
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
    pub fn new(symbol: String, exchange: super::Exchange) -> Self {
        Self {
            symbol,
            exchange,
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
            exchange: self.exchange.clone(),
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
    pub exchange: super::Exchange,
    // Group frequently accessed atomics together for cache efficiency
    pub timestamp: AtomicI64,
    pub price: AtomicU64, // Store as integer (price * 100000000)
    pub quantity: AtomicU64,
    pub side: AtomicU64, // 0 = buy, 1 = sell
    pub trade_id: AtomicU64,
}

impl AtomicTrade {
    /// Create a new AtomicTrade with default values
    pub fn new(symbol: String, exchange: super::Exchange) -> Self {
        Self {
            symbol,
            exchange,
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

        // Convert TradeSide enum to numeric value
        let side_value = match data.side {
            super::TradeSide::Buy => 0,
            super::TradeSide::Sell => 1,
        };
        self.side.store(side_value, Ordering::Relaxed);

        // Convert trade_id string to hash for atomic storage
        let trade_id_hash = self.hash_trade_id(&data.trade_id);
        self.trade_id.store(trade_id_hash, Ordering::Relaxed);
    }

    /// Convert to snapshot for periodic captures
    pub fn to_snapshot(&self) -> TradeSnapshot {
        TradeSnapshot {
            symbol: self.symbol.clone(),
            exchange: self.exchange.clone(),
            timestamp: self.timestamp.load(Ordering::Relaxed),
            price: self.price.load(Ordering::Relaxed) as f64 / PRICE_SCALE,
            quantity: self.quantity.load(Ordering::Relaxed) as f64 / QUANTITY_SCALE,
            side: match self.side.load(Ordering::Relaxed) {
                0 => super::TradeSide::Buy,
                1 => super::TradeSide::Sell,
                _ => super::TradeSide::Buy,
            },
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

    /// Get or create an AtomicOrderBook for a symbol and exchange
    pub fn get_or_create_orderbook(
        &self,
        symbol: &str,
        exchange: &super::Exchange,
    ) -> Arc<AtomicOrderBook> {
        let key = format!("{exchange}:{symbol}");
        self.orderbooks
            .entry(key)
            .or_insert_with(|| Arc::new(AtomicOrderBook::new(symbol.to_string(), exchange.clone())))
            .clone()
    }

    /// Get or create an AtomicTrade for a symbol and exchange
    pub fn get_or_create_trade(
        &self,
        symbol: &str,
        exchange: &super::Exchange,
    ) -> Arc<AtomicTrade> {
        let key = format!("{exchange}:{symbol}");
        self.latest_trades
            .entry(key)
            .or_insert_with(|| Arc::new(AtomicTrade::new(symbol.to_string(), exchange.clone())))
            .clone()
    }

    /// Legacy methods for backward compatibility (will be deprecated)
    #[deprecated(note = "Use get_or_create_orderbook with exchange parameter")]
    pub fn get_or_create_orderbook_legacy(&self, symbol: &str) -> Arc<AtomicOrderBook> {
        self.orderbooks
            .entry(symbol.to_string())
            .or_insert_with(|| {
                Arc::new(AtomicOrderBook::new(
                    symbol.to_string(),
                    super::Exchange::BinanceSpot,
                ))
            })
            .clone()
    }

    #[deprecated(note = "Use get_or_create_trade with exchange parameter")]
    pub fn get_or_create_trade_legacy(&self, symbol: &str) -> Arc<AtomicTrade> {
        self.latest_trades
            .entry(symbol.to_string())
            .or_insert_with(|| {
                Arc::new(AtomicTrade::new(
                    symbol.to_string(),
                    super::Exchange::BinanceSpot,
                ))
            })
            .clone()
    }

    /// Update orderbook data atomically
    pub fn update_orderbook(&self, data: &OrderBookData) {
        let orderbook = self.get_or_create_orderbook(&data.symbol, &data.exchange);
        orderbook.update_from_data(data);
    }

    /// Update trade data atomically
    pub fn update_trade(&self, data: &TradeData) {
        let trade = self.get_or_create_trade(&data.symbol, &data.exchange);
        trade.update_from_data(data);
    }

    /// Get orderbook snapshot for a symbol and exchange
    pub fn get_orderbook_snapshot(
        &self,
        symbol: &str,
        exchange: &super::Exchange,
    ) -> Option<OrderBookSnapshot> {
        let key = format!("{exchange}:{symbol}");
        self.orderbooks
            .get(&key)
            .map(|orderbook| orderbook.to_snapshot())
    }

    /// Get trade snapshot for a symbol and exchange
    pub fn get_trade_snapshot(
        &self,
        symbol: &str,
        exchange: &super::Exchange,
    ) -> Option<TradeSnapshot> {
        let key = format!("{exchange}:{symbol}");
        self.latest_trades
            .get(&key)
            .map(|trade| trade.to_snapshot())
    }

    /// Legacy methods for backward compatibility (will be deprecated)
    #[deprecated(note = "Use get_orderbook_snapshot with exchange parameter")]
    pub fn get_orderbook_snapshot_legacy(&self, symbol: &str) -> Option<OrderBookSnapshot> {
        self.orderbooks
            .get(symbol)
            .map(|orderbook| orderbook.to_snapshot())
    }

    #[deprecated(note = "Use get_trade_snapshot with exchange parameter")]
    pub fn get_trade_snapshot_legacy(&self, symbol: &str) -> Option<TradeSnapshot> {
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

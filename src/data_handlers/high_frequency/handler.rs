// High Frequency Handler
// "The fastest ravens in the realm - delivering messages with sub-microsecond speed"

use crate::types::{
    HighFrequencyStorage, OrderBookData, OrderBookSnapshot, TradeData, TradeSnapshot,
};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

use super::PerformanceStats;

/// High-frequency data ingestion handler with lock-free atomic operations
/// Designed for maximum throughput with sub-microsecond write performance
pub struct HighFrequencyHandler {
    /// Lock-free atomic storage for maximum throughput
    storage: Arc<HighFrequencyStorage>,
}

impl HighFrequencyHandler {
    /// Create a new HighFrequencyHandler with atomic storage
    pub fn new() -> Self {
        info!("⚡ High frequency ravens are taking flight...");
        Self {
            storage: Arc::new(HighFrequencyStorage::new()),
        }
    }

    /// Create a new HighFrequencyHandler with shared storage
    pub fn with_storage(storage: Arc<HighFrequencyStorage>) -> Self {
        info!("⚡ High frequency ravens joining the existing flock...");
        Self { storage }
    }

    /// Get reference to the underlying storage for sharing with other components
    pub fn get_storage(&self) -> Arc<HighFrequencyStorage> {
        Arc::clone(&self.storage)
    }

    /// Ingest orderbook data with lock-free atomic updates
    /// This function is designed for maximum throughput with direct memory updates
    /// No async overhead, no channel bottlenecks - pure speed
    /// Supports multiple exchanges by using exchange-qualified symbol keys
    pub fn ingest_orderbook_atomic(&self, symbol: &str, data: &OrderBookData) -> Result<()> {
        let start = Instant::now();

        // Validate input data
        if symbol.is_empty() {
            return Err(anyhow::anyhow!("Symbol cannot be empty"));
        }

        if data.symbol != symbol {
            warn!("Symbol mismatch: expected {}, got {}", symbol, data.symbol);
        }

        // Validate orderbook data integrity
        if data.bids.is_empty() && data.asks.is_empty() {
            return Err(anyhow::anyhow!("Orderbook data cannot be empty"));
        }

        // Validate price levels are sorted correctly
        if !self.validate_price_levels(&data.bids, &data.asks)? {
            return Err(anyhow::anyhow!("Invalid price level ordering"));
        }

        // Create exchange-qualified symbol key for multi-exchange support
        let exchange_symbol = format!("{}:{}", data.exchange, symbol);

        // Create a modified data structure with the exchange-qualified symbol
        let mut exchange_data = data.clone();
        exchange_data.symbol = exchange_symbol;

        // Perform lock-free atomic update - this is the critical path
        self.storage.update_orderbook(&exchange_data);

        let duration = start.elapsed();
        debug!(
            "📊 Orderbook atomic update for {} completed in {:?} (sequence: {})",
            symbol, duration, data.sequence
        );

        // Log warning if update took longer than expected (should be sub-microsecond)
        if duration.as_nanos() > 1000 {
            warn!(
                "Orderbook update for {} took {:?} - exceeds sub-microsecond target",
                symbol, duration
            );
        }

        Ok(())
    }

    /// Ingest trade data with lock-free atomic updates
    /// This function is designed for maximum throughput with direct memory updates
    /// No async overhead, no channel bottlenecks - pure speed
    /// Supports multiple exchanges by using exchange-qualified symbol keys
    pub fn ingest_trade_atomic(&self, symbol: &str, data: &TradeData) -> Result<()> {
        let start = Instant::now();

        // Validate input data
        if symbol.is_empty() {
            return Err(anyhow::anyhow!("Symbol cannot be empty"));
        }

        if data.symbol != symbol {
            warn!("Symbol mismatch: expected {}, got {}", symbol, data.symbol);
        }

        // Validate trade data
        if data.price <= 0.0 {
            return Err(anyhow::anyhow!("Trade price must be positive"));
        }

        if data.quantity <= 0.0 {
            return Err(anyhow::anyhow!("Trade quantity must be positive"));
        }

        // TradeSide enum already validates the side, no need for string matching

        if data.trade_id.is_empty() {
            return Err(anyhow::anyhow!("Trade ID cannot be empty"));
        }

        // Create exchange-qualified symbol key for multi-exchange support
        let exchange_symbol = format!("{}:{}", data.exchange, symbol);

        // Create a modified data structure with the exchange-qualified symbol
        let mut exchange_data = data.clone();
        exchange_data.symbol = exchange_symbol;

        // Perform lock-free atomic update - this is the critical path
        self.storage.update_trade(&exchange_data);

        let duration = start.elapsed();
        debug!(
            "💱 Trade atomic update for {} completed in {:?} (price: {}, qty: {})",
            symbol, duration, data.price, data.quantity
        );

        // Log warning if update took longer than expected (should be sub-microsecond)
        if duration.as_nanos() > 1000 {
            warn!(
                "Trade update for {} took {:?} - exceeds sub-microsecond target",
                symbol, duration
            );
        }

        Ok(())
    }

    /// Capture atomic snapshot of current orderbook state
    /// This function performs atomic reads without blocking writes
    pub fn capture_orderbook_snapshot(&self, symbol: &str) -> Result<OrderBookSnapshot> {
        let start = Instant::now();

        let snapshot = self
            .storage
            .get_orderbook_snapshot(symbol)
            .with_context(|| format!("No orderbook data found for symbol: {symbol}"))?;

        let duration = start.elapsed();
        debug!(
            "📸 Orderbook snapshot captured for {} in {:?}",
            symbol, duration
        );

        Ok(snapshot)
    }

    /// Capture atomic snapshot of latest trade state
    /// This function performs atomic reads without blocking writes
    pub fn capture_trade_snapshot(&self, symbol: &str) -> Result<TradeSnapshot> {
        let start = Instant::now();

        let snapshot = self
            .storage
            .get_trade_snapshot(symbol)
            .with_context(|| format!("No trade data found for symbol: {symbol}"))?;

        let duration = start.elapsed();
        debug!(
            "📸 Trade snapshot captured for {} in {:?}",
            symbol, duration
        );

        Ok(snapshot)
    }

    /// Get all symbols with orderbook data
    pub fn get_orderbook_symbols(&self) -> Vec<String> {
        self.storage.get_orderbook_symbols()
    }

    /// Get all symbols with trade data
    pub fn get_trade_symbols(&self) -> Vec<String> {
        self.storage.get_trade_symbols()
    }

    /// Validate that price levels are correctly ordered
    /// Bids should be in descending order (highest first)
    /// Asks should be in ascending order (lowest first)
    #[cfg(test)]
    pub fn validate_price_levels(&self, bids: &[(f64, f64)], asks: &[(f64, f64)]) -> Result<bool> {
        // Validate bids are in descending order
        for window in bids.windows(2) {
            if window[0].0 < window[1].0 {
                error!(
                    "Bids not in descending order: {} < {}",
                    window[0].0, window[1].0
                );
                return Ok(false);
            }
        }

        // Validate asks are in ascending order
        for window in asks.windows(2) {
            if window[0].0 > window[1].0 {
                error!(
                    "Asks not in ascending order: {} > {}",
                    window[0].0, window[1].0
                );
                return Ok(false);
            }
        }

        // Validate spread (best ask >= best bid)
        if let (Some(best_bid), Some(best_ask)) = (bids.first(), asks.first()) {
            if best_ask.0 < best_bid.0 {
                error!(
                    "Invalid spread: best ask ({}) < best bid ({})",
                    best_ask.0, best_bid.0
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    #[cfg(not(test))]
    fn validate_price_levels(&self, bids: &[(f64, f64)], asks: &[(f64, f64)]) -> Result<bool> {
        // Validate bids are in descending order
        for window in bids.windows(2) {
            if window[0].0 < window[1].0 {
                error!(
                    "Bids not in descending order: {} < {}",
                    window[0].0, window[1].0
                );
                return Ok(false);
            }
        }

        // Validate asks are in ascending order
        for window in asks.windows(2) {
            if window[0].0 > window[1].0 {
                error!(
                    "Asks not in ascending order: {} > {}",
                    window[0].0, window[1].0
                );
                return Ok(false);
            }
        }

        // Validate spread (best ask >= best bid)
        if let (Some(best_bid), Some(best_ask)) = (bids.first(), asks.first()) {
            if best_ask.0 < best_bid.0 {
                error!(
                    "Invalid spread: best ask ({}) < best bid ({})",
                    best_ask.0, best_bid.0
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Get performance statistics for monitoring
    pub fn get_performance_stats(&self) -> PerformanceStats {
        let orderbook_count = self.storage.get_orderbook_symbols().len();
        let trade_count = self.storage.get_trade_symbols().len();

        PerformanceStats {
            orderbook_symbols: orderbook_count,
            trade_symbols: trade_count,
        }
    }

    /// Get all exchanges currently active in the system
    pub fn get_active_exchanges(&self) -> Vec<String> {
        let mut exchanges = std::collections::HashSet::new();

        // Extract exchanges from orderbook symbols
        for symbol in self.storage.get_orderbook_symbols() {
            if let Some(exchange) = symbol.split(':').next() {
                exchanges.insert(exchange.to_string());
            }
        }

        // Extract exchanges from trade symbols
        for symbol in self.storage.get_trade_symbols() {
            if let Some(exchange) = symbol.split(':').next() {
                exchanges.insert(exchange.to_string());
            }
        }

        exchanges.into_iter().collect()
    }

    /// Get symbols for a specific exchange
    pub fn get_symbols_for_exchange(&self, exchange: &str) -> Vec<String> {
        let prefix = format!("{exchange}:");
        let mut symbols = Vec::new();

        // Get orderbook symbols for this exchange
        for symbol in self.storage.get_orderbook_symbols() {
            if symbol.starts_with(&prefix) {
                if let Some(base_symbol) = symbol.strip_prefix(&prefix) {
                    symbols.push(base_symbol.to_string());
                }
            }
        }

        // Get trade symbols for this exchange (deduplicate)
        for symbol in self.storage.get_trade_symbols() {
            if symbol.starts_with(&prefix) {
                if let Some(base_symbol) = symbol.strip_prefix(&prefix) {
                    if !symbols.contains(&base_symbol.to_string()) {
                        symbols.push(base_symbol.to_string());
                    }
                }
            }
        }

        symbols
    }

    /// Capture orderbook snapshot for a specific exchange and symbol
    pub fn capture_orderbook_snapshot_for_exchange(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> Result<OrderBookSnapshot> {
        let exchange_symbol = format!("{exchange}:{symbol}");
        self.capture_orderbook_snapshot(&exchange_symbol)
    }

    /// Capture trade snapshot for a specific exchange and symbol
    pub fn capture_trade_snapshot_for_exchange(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> Result<TradeSnapshot> {
        let exchange_symbol = format!("{exchange}:{symbol}");
        self.capture_trade_snapshot(&exchange_symbol)
    }
}

impl Default for HighFrequencyHandler {
    fn default() -> Self {
        Self::new()
    }
}

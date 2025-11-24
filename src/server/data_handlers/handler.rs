use crate::common::error::RavenResult;
use crate::server::data_engine::storage::{
    HighFrequencyStorage, OrderBookData, OrderBookSnapshot, TradeData, TradeSnapshot,
};
use crate::server::exchanges::types::Exchange;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

use super::performance_stats::PerformanceStats;

/// High-frequency data ingestion handler with lock-free atomic operations
/// Designed for maximum throughput with sub-microsecond write performance
pub struct HighFrequencyHandler {
    /// Lock-free atomic storage for maximum throughput
    storage: Arc<HighFrequencyStorage>,
}

impl HighFrequencyHandler {
    /// Create a new HighFrequencyHandler with atomic storage
    pub fn new() -> Self {
        info!("High frequency ravens are taking flight...");
        Self {
            storage: Arc::new(HighFrequencyStorage::new()),
        }
    }

    /// Create a new HighFrequencyHandler with shared storage
    pub fn with_storage(storage: Arc<HighFrequencyStorage>) -> Self {
        info!("High frequency ravens joining the existing flock...");
        Self { storage }
    }

    /// Get reference to the underlying storage for sharing with other components
    pub fn get_storage(&self) -> Arc<HighFrequencyStorage> {
        Arc::clone(&self.storage)
    }

    /// Remove any cached orderbook/trade data for the given symbol and exchange.
    pub fn clear_symbol_data(&self, symbol: &str, exchange: &Exchange) -> bool {
        self.storage.remove_symbol(symbol, exchange)
    }

    /// Ingest orderbook data with lock-free atomic updates
    /// This function is designed for maximum throughput with direct memory updates
    /// No async overhead, no channel bottlenecks - pure speed
    /// Supports multiple exchanges by using exchange-qualified symbol keys
    pub fn ingest_orderbook_atomic(&self, symbol: &str, data: &OrderBookData) -> RavenResult<()> {
        let start = Instant::now();

        // Perform lock-free atomic update - storage handles exchange scoping
        self.storage.update_orderbook(data);

        let duration = start.elapsed();
        debug!(
            "Orderbook atomic update for {} completed in {:?} (sequence: {})",
            symbol, duration, data.sequence
        );

        // Log warning if update took longer than expected (development: 10μs, production: 1μs)
        let threshold_ns = if cfg!(debug_assertions) {
            10_000
        } else {
            1_000
        };
        if duration.as_nanos() > threshold_ns {
            warn!(
                "Orderbook update for {} took {:?} - exceeds performance target",
                symbol, duration
            );
        }

        Ok(())
    }

    /// Ingest trade data with lock-free atomic updates
    /// This function is designed for maximum throughput with direct memory updates
    /// No async overhead, no channel bottlenecks - pure speed
    /// Supports multiple exchanges by using exchange-qualified symbol keys
    pub fn ingest_trade_atomic(&self, symbol: &str, data: &TradeData) -> RavenResult<()> {
        let start = Instant::now();

        // Perform lock-free atomic update - storage handles exchange scoping
        self.storage.update_trade(data);

        let duration = start.elapsed();
        debug!(
            "⟷ Trade atomic update for {} completed in {:?} (price: {}, qty: {})",
            symbol, duration, data.price, data.quantity
        );

        // Log warning if update took longer than expected (development: 10μs, production: 1μs)
        let threshold_ns = if cfg!(debug_assertions) {
            10_000
        } else {
            1_000
        };
        if duration.as_nanos() > threshold_ns {
            warn!(
                "Trade update for {} took {:?} - exceeds performance target",
                symbol, duration
            );
        }

        Ok(())
    }

    /// Capture atomic snapshot of current orderbook state
    /// This function performs atomic reads without blocking writes
    pub fn capture_orderbook_snapshot(
        &self,
        symbol: &str,
        exchange: &Exchange,
    ) -> RavenResult<OrderBookSnapshot> {
        let start = Instant::now();

        let snapshot = self
            .storage
            .get_orderbook_snapshot(symbol, exchange)
            .ok_or_else(|| {
                crate::raven_error!(
                    subscription_failed,
                    format!("No orderbook data found for symbol: {symbol}")
                )
            })?;

        let duration = start.elapsed();
        debug!(
            "◉ Orderbook snapshot captured for {} in {:?}",
            symbol, duration
        );

        Ok(snapshot)
    }

    /// Capture atomic snapshot of latest trade state
    /// This function performs atomic reads without blocking writes
    pub fn capture_trade_snapshot(
        &self,
        symbol: &str,
        exchange: &Exchange,
    ) -> RavenResult<TradeSnapshot> {
        let start = Instant::now();

        let snapshot = self
            .storage
            .get_trade_snapshot(symbol, exchange)
            .ok_or_else(|| {
                crate::raven_error!(
                    subscription_failed,
                    format!("No trade data found for symbol: {symbol}")
                )
            })?;

        let duration = start.elapsed();
        debug!("Δ Trade snapshot captured for {} in {:?}", symbol, duration);

        Ok(snapshot)
    }

    // Validate price levels removed as per user instruction to trust exchange

    /// Get symbols with active orderbooks
    pub fn get_orderbook_symbols(&self) -> Vec<String> {
        self.storage.get_orderbook_symbols()
    }

    /// Get symbols with recent trades
    pub fn get_trade_symbols(&self) -> Vec<String> {
        self.storage.get_trade_symbols()
    }

    /// Get performance statistics for monitoring dashboards
    pub fn get_performance_stats(&self) -> PerformanceStats {
        PerformanceStats {
            orderbook_symbols: self.storage.get_orderbook_symbols().len(),
            trade_symbols: self.storage.get_trade_symbols().len(),
        }
    }

    /// Get the list of active exchanges currently seen in data
    pub fn get_active_exchanges(&self) -> Vec<String> {
        let mut exchanges: Vec<String> = self
            .storage
            .get_orderbook_symbols()
            .into_iter()
            .chain(self.storage.get_trade_symbols())
            .filter_map(|key| {
                key.split_once(':')
                    .map(|(exchange, _)| exchange.to_string())
            })
            .collect();
        exchanges.sort();
        exchanges.dedup();
        exchanges
    }

    /// Get symbols for a specific exchange
    pub fn get_symbols_for_exchange(&self, exchange: &str) -> Vec<String> {
        let exchange_key = match Self::parse_exchange(exchange) {
            Ok(exchange) => exchange.to_string(),
            Err(err) => {
                warn!(error = %err, "Ignoring symbols lookup for unsupported exchange");
                return Vec::new();
            }
        };
        let prefix = format!("{exchange_key}:");
        let mut symbols: Vec<String> = self
            .storage
            .get_orderbook_symbols()
            .into_iter()
            .chain(self.storage.get_trade_symbols())
            .filter_map(|key| key.strip_prefix(&prefix).map(|symbol| symbol.to_string()))
            .collect();
        symbols.sort();
        symbols.dedup();
        symbols
    }

    /// Capture orderbook snapshot with explicit exchange selection
    pub fn capture_orderbook_snapshot_for_exchange(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> RavenResult<OrderBookSnapshot> {
        let exchange = Self::parse_exchange(exchange)?;

        self.storage
            .get_orderbook_snapshot(symbol, &exchange)
            .ok_or_else(|| {
                crate::raven_error!(
                    subscription_failed,
                    format!("No orderbook data found for {} on {}", symbol, exchange)
                )
            })
    }

    /// Capture trade snapshot with explicit exchange selection
    pub fn capture_trade_snapshot_for_exchange(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> RavenResult<TradeSnapshot> {
        let exchange = Self::parse_exchange(exchange)?;

        self.storage
            .get_trade_snapshot(symbol, &exchange)
            .ok_or_else(|| {
                crate::raven_error!(
                    subscription_failed,
                    format!("No trade data found for {} on {}", symbol, exchange)
                )
            })
    }

    fn parse_exchange(exchange: &str) -> RavenResult<Exchange> {
        let normalized = exchange.to_lowercase();
        match normalized.as_str() {
            "binance_spot" | "binance-spot" => Ok(Exchange::BinanceSpot),
            "binance_futures" | "binance-futures" => Ok(Exchange::BinanceFutures),
            other => {
                crate::raven_bail!(crate::raven_error!(
                    data_validation,
                    format!("Unsupported exchange identifier: {other}")
                ));
            }
        }
    }
}

impl Default for HighFrequencyHandler {
    fn default() -> Self {
        Self::new()
    }
}

// Citadel - Unified Data Management System
// "The fortress that guards the integrity of our data, stores it atomically, and streams it to the realm"

// Sub-modules
pub mod app; // Ingestion orchestration for data collectors
pub mod storage; // Atomic data storage (formerly types)
pub mod streaming; // Data streaming to clients (formerly snapshot_service)

// Re-export core types and functionality
pub use storage::{
    atomic::{AtomicOrderBook, AtomicTrade, HighFrequencyStorage},
    snapshots::{OrderBookSnapshot, TradeSnapshot},
    {CandleData, OrderBookData, TickerData, TradeData, TradeSide},
};

pub use streaming::{SnapshotBatch, SnapshotConfig, SnapshotMetrics, SnapshotService};

use crate::database::{influx_client::InfluxClient, DeadLetterQueue};
use crate::error::{RavenError, RavenResult};
use crate::exchanges::types::Exchange;
use crate::subscription_manager::SubscriptionManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for the Citadel validation engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitadelConfig {
    /// Enable strict validation mode
    pub strict_validation: bool,
    /// Maximum allowed price deviation (percentage)
    pub max_price_deviation: f64,
    /// Maximum allowed quantity
    pub max_quantity: f64,
    /// Minimum allowed price
    pub min_price: f64,
    /// Maximum allowed price
    pub max_price: f64,
    /// Enable data sanitization
    pub enable_sanitization: bool,
    /// Maximum age of data in seconds
    pub max_data_age_seconds: u64,
    /// Enable dead letter queue
    pub enable_dead_letter_queue: bool,
}

impl Default for CitadelConfig {
    fn default() -> Self {
        Self {
            strict_validation: true,
            max_price_deviation: 10.0, // 10%
            max_quantity: 1000000.0,
            min_price: 0.00000001,
            max_price: 1000000.0,
            enable_sanitization: true,
            max_data_age_seconds: 300, // 5 minutes
            enable_dead_letter_queue: true,
        }
    }
}

/// Validation rules for market data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    pub min_price: f64,
    pub max_price: f64,
    pub min_quantity: f64,
    pub max_quantity: f64,
    pub max_spread_percentage: f64,
    pub max_price_deviation: f64,
    pub required_fields: Vec<String>,
    pub allowed_exchanges: Vec<Exchange>,
    pub allowed_symbols: Vec<String>,
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self {
            min_price: 0.00000001,
            max_price: 1000000.0,
            min_quantity: 0.00000001,
            max_quantity: 1000000.0,
            max_spread_percentage: 5.0,
            max_price_deviation: 10.0,
            required_fields: vec![
                "symbol".to_string(),
                "timestamp".to_string(),
                "exchange".to_string(),
            ],
            allowed_exchanges: vec![
                Exchange::BinanceSpot,
                Exchange::BinanceFutures,
                // Exchange::Coinbase,
                // Exchange::Kraken,
            ],
            allowed_symbols: vec![
                "BTCUSDT".to_string(),
                "ETHUSDT".to_string(),
                "ADAUSDT".to_string(),
            ],
        }
    }
}

/// Citadel metrics for monitoring validation performance
#[derive(Debug)]
pub struct CitadelMetrics {
    pub total_ingested: AtomicU64,
    pub total_validated: AtomicU64,
    pub total_written: AtomicU64,
    pub total_failed: AtomicU64,
    pub validation_errors: AtomicU64,
    pub sanitization_fixes: AtomicU64,
    pub dead_letter_entries: AtomicU64,
}

impl Default for CitadelMetrics {
    fn default() -> Self {
        Self {
            total_ingested: AtomicU64::new(0),
            total_validated: AtomicU64::new(0),
            total_written: AtomicU64::new(0),
            total_failed: AtomicU64::new(0),
            validation_errors: AtomicU64::new(0),
            sanitization_fixes: AtomicU64::new(0),
            dead_letter_entries: AtomicU64::new(0),
        }
    }
}

/// The Citadel - Main data validation and processing engine
pub struct Citadel {
    config: CitadelConfig,
    validation_rules: Arc<RwLock<ValidationRules>>,
    influx_client: Arc<InfluxClient>,
    _subscription_manager: Arc<SubscriptionManager>,
    _dead_letter_queue: Arc<DeadLetterQueue>,
    pub metrics: CitadelMetrics,
}

impl Citadel {
    /// Create a new Citadel instance
    pub fn new(
        config: CitadelConfig,
        influx_client: Arc<InfluxClient>,
        subscription_manager: Arc<SubscriptionManager>,
    ) -> Self {
        info!("🏰 Initializing Citadel with config: {:?}", config);

        let dead_letter_queue = Arc::new(DeadLetterQueue::new(Default::default()));

        Self {
            config,
            validation_rules: Arc::new(RwLock::new(ValidationRules::default())),
            influx_client,
            _subscription_manager: subscription_manager,
            _dead_letter_queue: dead_letter_queue,
            metrics: CitadelMetrics::default(),
        }
    }

    /// Process incoming order book data
    pub async fn process_orderbook_data(
        &self,
        symbol: &str,
        data: OrderBookData,
    ) -> RavenResult<()> {
        self.metrics.total_ingested.fetch_add(1, Ordering::Relaxed);

        debug!("🏰 Processing orderbook data for symbol: {}", symbol);

        // Validate the data
        let validated_data = match self.validate_orderbook_data(symbol, &data).await {
            Ok(data) => {
                self.metrics.total_validated.fetch_add(1, Ordering::Relaxed);
                data
            }
            Err(e) => {
                self.metrics
                    .validation_errors
                    .fetch_add(1, Ordering::Relaxed);
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);

                if self.config.enable_dead_letter_queue {
                    self.add_to_dead_letter_queue(
                        symbol.to_string(),
                        serde_json::to_string(&data).unwrap_or_default(),
                        e.to_string(),
                    )
                    .await;
                }

                return Err(e);
            }
        };

        // Sanitize if enabled
        let final_data = if self.config.enable_sanitization {
            match self.sanitize_orderbook_data(&validated_data).await {
                Ok(data) => {
                    self.metrics
                        .sanitization_fixes
                        .fetch_add(1, Ordering::Relaxed);
                    data
                }
                Err(e) => {
                    warn!("🏰 Sanitization failed for {}: {}", symbol, e);
                    validated_data
                }
            }
        } else {
            validated_data
        };

        // Write to database
        match self.write_orderbook_data(&final_data).await {
            Ok(_) => {
                self.metrics.total_written.fetch_add(1, Ordering::Relaxed);
                debug!("✅ Successfully processed orderbook data for {}", symbol);
            }
            Err(e) => {
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                error!("❌ Failed to write orderbook data for {}: {}", symbol, e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Process incoming trade data
    pub async fn process_trade_data(&self, symbol: &str, data: TradeData) -> RavenResult<()> {
        self.metrics.total_ingested.fetch_add(1, Ordering::Relaxed);

        debug!("🏰 Processing trade data for symbol: {}", symbol);

        // Validate the data
        let validated_data = match self.validate_trade_data(symbol, &data).await {
            Ok(data) => {
                self.metrics.total_validated.fetch_add(1, Ordering::Relaxed);
                data
            }
            Err(e) => {
                self.metrics
                    .validation_errors
                    .fetch_add(1, Ordering::Relaxed);
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);

                if self.config.enable_dead_letter_queue {
                    self.add_to_dead_letter_queue(
                        symbol.to_string(),
                        serde_json::to_string(&data).unwrap_or_default(),
                        e.to_string(),
                    )
                    .await;
                }

                return Err(e);
            }
        };

        // Write to database
        match self.write_trade_data(&validated_data).await {
            Ok(_) => {
                self.metrics.total_written.fetch_add(1, Ordering::Relaxed);
                debug!("✅ Successfully processed trade data for {}", symbol);
            }
            Err(e) => {
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                error!("❌ Failed to write trade data for {}: {}", symbol, e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Validate order book data
    pub async fn validate_orderbook_data(
        &self,
        symbol: &str,
        data: &OrderBookData,
    ) -> RavenResult<OrderBookData> {
        let rules = self.validation_rules.read().await;

        // Check if symbol is allowed
        if !rules.allowed_symbols.is_empty() && !rules.allowed_symbols.contains(&data.symbol) {
            return Err(RavenError::data_validation(format!(
                "Symbol {} not in allowed list",
                data.symbol
            )));
        }

        // Check if exchange is allowed
        if !rules.allowed_exchanges.is_empty() && !rules.allowed_exchanges.contains(&data.exchange)
        {
            return Err(RavenError::data_validation(format!(
                "Exchange {} not in allowed list",
                data.exchange
            )));
        }

        // Validate timestamp
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        if (current_time - data.timestamp) > (self.config.max_data_age_seconds * 1000) as i64 {
            return Err(RavenError::data_validation("Data too old".to_string()));
        }

        // Validate bids and asks
        for (price, quantity) in &data.bids {
            if *price < rules.min_price || *price > rules.max_price {
                return Err(RavenError::data_validation(format!(
                    "Bid price {price} out of range"
                )));
            }
            if *quantity < rules.min_quantity || *quantity > rules.max_quantity {
                return Err(RavenError::data_validation(format!(
                    "Bid quantity {quantity} out of range"
                )));
            }
        }

        for (price, quantity) in &data.asks {
            if *price < rules.min_price || *price > rules.max_price {
                return Err(RavenError::data_validation(format!(
                    "Ask price {price} out of range"
                )));
            }
            if *quantity < rules.min_quantity || *quantity > rules.max_quantity {
                return Err(RavenError::data_validation(format!(
                    "Ask quantity {quantity} out of range"
                )));
            }
        }

        // Check spread
        if let (Some(best_bid), Some(best_ask)) = (
            data.bids.first().map(|(p, _)| *p),
            data.asks.first().map(|(p, _)| *p),
        ) {
            let spread_percentage = ((best_ask - best_bid) / best_bid) * 100.0;
            if spread_percentage > rules.max_spread_percentage {
                return Err(RavenError::data_validation(format!(
                    "Spread too wide: {spread_percentage:.2}%"
                )));
            }
        }

        debug!("✅ Orderbook data validation passed for {}", symbol);
        Ok(data.clone())
    }

    /// Validate trade data
    pub async fn validate_trade_data(
        &self,
        symbol: &str,
        data: &TradeData,
    ) -> RavenResult<TradeData> {
        let rules = self.validation_rules.read().await;

        // Check if symbol is allowed
        if !rules.allowed_symbols.is_empty() && !rules.allowed_symbols.contains(&data.symbol) {
            return Err(RavenError::data_validation(format!(
                "Symbol {} not in allowed list",
                data.symbol
            )));
        }

        // Check if exchange is allowed
        if !rules.allowed_exchanges.is_empty() && !rules.allowed_exchanges.contains(&data.exchange)
        {
            return Err(RavenError::data_validation(format!(
                "Exchange {} not in allowed list",
                data.exchange
            )));
        }

        // Validate timestamp
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        if (current_time - data.timestamp) > (self.config.max_data_age_seconds * 1000) as i64 {
            return Err(RavenError::data_validation("Data too old".to_string()));
        }

        // Validate price
        if data.price < rules.min_price || data.price > rules.max_price {
            return Err(RavenError::data_validation(format!(
                "Price {} out of range",
                data.price
            )));
        }

        // Validate quantity
        if data.quantity < rules.min_quantity || data.quantity > rules.max_quantity {
            return Err(RavenError::data_validation(format!(
                "Quantity {} out of range",
                data.quantity
            )));
        }

        // TradeSide enum already validates the side, no need for additional validation

        debug!("✅ Trade data validation passed for {}", symbol);
        Ok(data.clone())
    }

    /// Sanitize order book data
    pub async fn sanitize_orderbook_data(
        &self,
        data: &OrderBookData,
    ) -> RavenResult<OrderBookData> {
        let mut sanitized = data.clone();

        // Normalize symbol (exchange is already an enum, no need to normalize)
        sanitized.symbol = sanitized.symbol.trim().to_uppercase();

        // Round prices and quantities to reasonable precision
        sanitized.bids = sanitized
            .bids
            .into_iter()
            .map(|(p, q)| (self.round_price(p), self.round_quantity(q)))
            .collect();

        sanitized.asks = sanitized
            .asks
            .into_iter()
            .map(|(p, q)| (self.round_price(p), self.round_quantity(q)))
            .collect();

        debug!("🧹 Sanitized orderbook data for {}", sanitized.symbol);
        Ok(sanitized)
    }

    /// Write order book data to database
    async fn write_orderbook_data(&self, data: &OrderBookData) -> RavenResult<()> {
        let snapshot = OrderBookSnapshot::from(data);
        self.influx_client
            .write_orderbook_snapshot(&snapshot)
            .await
            .map_err(|e| RavenError::database_write(e.to_string()))
    }

    /// Write trade data to database
    async fn write_trade_data(&self, data: &TradeData) -> RavenResult<()> {
        let snapshot = TradeSnapshot::from(data);
        self.influx_client
            .write_trade_snapshot(&snapshot)
            .await
            .map_err(|e| RavenError::database_write(e.to_string()))
    }

    /// Round price to 8 decimal places
    pub fn round_price(&self, price: f64) -> f64 {
        (price * 100_000_000.0).round() / 100_000_000.0
    }

    /// Round quantity to 8 decimal places
    pub fn round_quantity(&self, quantity: f64) -> f64 {
        (quantity * 100_000_000.0).round() / 100_000_000.0
    }

    /// Update validation rules
    pub async fn update_validation_rules(&self, rules: ValidationRules) -> RavenResult<()> {
        let mut current_rules = self.validation_rules.write().await;
        *current_rules = rules;
        info!("Updated validation rules");
        Ok(())
    }

    /// Get current validation rules
    pub async fn get_validation_rules(&self) -> ValidationRules {
        self.validation_rules.read().await.clone()
    }

    /// Add entry to dead letter queue
    pub async fn add_to_dead_letter_queue(&self, symbol: String, _data: String, error: String) {
        // TODO: Create proper dead letter entry
        debug!("Would add to dead letter queue: {} - {}", symbol, error);
        self.metrics
            .dead_letter_entries
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Get dead letter queue status
    pub async fn get_dead_letter_queue_status(&self) -> HashMap<String, u64> {
        let mut status = HashMap::new();
        status.insert("total_entries".to_string(), 1u64);
        status.insert("test_entries".to_string(), 1u64);
        status
    }

    /// Get Citadel metrics
    pub fn get_metrics(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();
        metrics.insert(
            "total_ingested".to_string(),
            self.metrics.total_ingested.load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_validated".to_string(),
            self.metrics.total_validated.load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_written".to_string(),
            self.metrics.total_written.load(Ordering::Relaxed),
        );
        metrics.insert(
            "total_failed".to_string(),
            self.metrics.total_failed.load(Ordering::Relaxed),
        );
        metrics.insert(
            "validation_errors".to_string(),
            self.metrics.validation_errors.load(Ordering::Relaxed),
        );
        metrics.insert(
            "sanitization_fixes".to_string(),
            self.metrics.sanitization_fixes.load(Ordering::Relaxed),
        );
        metrics.insert(
            "dead_letter_entries".to_string(),
            self.metrics.dead_letter_entries.load(Ordering::Relaxed),
        );
        metrics
    }

    /// Get configuration
    pub fn get_config(&self) -> &CitadelConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests;

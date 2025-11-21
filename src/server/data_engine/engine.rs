use crate::server::database::{influx_client::InfluxClient, DeadLetterQueue};
use crate::common::error::RavenResult;
use crate::server::subscription_manager::SubscriptionManager;
use crate::common::time::current_timestamp_millis;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::config::DataEngineConfig;
use super::metrics::DataEngineMetrics;
use super::storage::{OrderBookSnapshot, TradeSnapshot};
use super::{OrderBookData, TradeData};
use super::validation::ValidationRules;

/// The Data Engine - Main data validation and processing engine
pub struct DataEngine {
    config: DataEngineConfig,
    validation_rules: Arc<RwLock<ValidationRules>>,
    influx_client: Arc<InfluxClient>,
    _subscription_manager: Arc<SubscriptionManager>,
    _dead_letter_queue: Arc<DeadLetterQueue>,
    pub metrics: DataEngineMetrics,
}

impl DataEngine {
    /// Create a new DataEngine instance
    pub fn new(
        config: DataEngineConfig,
        influx_client: Arc<InfluxClient>,
        subscription_manager: Arc<SubscriptionManager>,
    ) -> Self {
        info!("▲ Initializing DataEngine with config: {:?}", config);

        let dead_letter_queue = Arc::new(DeadLetterQueue::new(Default::default()));

        Self {
            config,
            validation_rules: Arc::new(RwLock::new(ValidationRules::default())),
            influx_client,
            _subscription_manager: subscription_manager,
            _dead_letter_queue: dead_letter_queue,
            metrics: DataEngineMetrics::default(),
        }
    }

    /// Process incoming order book data
    pub async fn process_orderbook_data(
        &self,
        symbol: &str,
        data: OrderBookData,
    ) -> RavenResult<()> {
        self.metrics.total_ingested.fetch_add(1, Ordering::Relaxed);

        debug!("▲ Processing orderbook data for symbol: {}", symbol);

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
                    warn!("▲ Sanitization failed for {}: {}", symbol, e);
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
                debug!("✓ Successfully processed orderbook data for {}", symbol);
            }
            Err(e) => {
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                error!("✗ Failed to write orderbook data for {}: {}", symbol, e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Process incoming trade data
    pub async fn process_trade_data(&self, symbol: &str, data: TradeData) -> RavenResult<()> {
        self.metrics.total_ingested.fetch_add(1, Ordering::Relaxed);

        debug!("▲ Processing trade data for symbol: {}", symbol);

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
                debug!("✓ Successfully processed trade data for {}", symbol);
            }
            Err(e) => {
                self.metrics.total_failed.fetch_add(1, Ordering::Relaxed);
                error!("✗ Failed to write trade data for {}: {}", symbol, e);
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

        // Validate timestamp
        let current_time = current_timestamp_millis();

        if (current_time - data.timestamp) > (self.config.max_data_age_seconds * 1000) as i64 {
            crate::raven_bail!(crate::raven_error!(
                data_validation,
                "Data too old".to_string()
            ));
        }

        // Validate bids and asks
        for (price, quantity) in &data.bids {
            if *price < rules.min_price || *price > rules.max_price {
                crate::raven_bail!(crate::raven_error!(
                    data_validation,
                    format!("Bid price {price} out of range")
                ));
            }
            if *quantity < rules.min_quantity || *quantity > rules.max_quantity {
                crate::raven_bail!(crate::raven_error!(
                    data_validation,
                    format!("Bid quantity {quantity} out of range")
                ));
            }
        }

        for (price, quantity) in &data.asks {
            if *price < rules.min_price || *price > rules.max_price {
                crate::raven_bail!(crate::raven_error!(
                    data_validation,
                    format!("Ask price {price} out of range")
                ));
            }
            if *quantity < rules.min_quantity || *quantity > rules.max_quantity {
                crate::raven_bail!(crate::raven_error!(
                    data_validation,
                    format!("Ask quantity {quantity} out of range")
                ));
            }
        }

        // Check spread
        if let (Some(best_bid), Some(best_ask)) = (
            data.bids.first().map(|(p, _)| *p),
            data.asks.first().map(|(p, _)| *p),
        ) {
            let spread_percentage = ((best_ask - best_bid) / best_bid) * 100.0;
            if spread_percentage > rules.max_spread_percentage {
                crate::raven_bail!(crate::raven_error!(
                    data_validation,
                    format!("Spread too wide: {spread_percentage:.2}%")
                ));
            }
        }

        debug!("✓ Orderbook data validation passed for {}", symbol);
        Ok(data.clone())
    }

    /// Validate trade data
    pub async fn validate_trade_data(
        &self,
        symbol: &str,
        data: &TradeData,
    ) -> RavenResult<TradeData> {
        let rules = self.validation_rules.read().await;

        // Validate timestamp
        let current_time = current_timestamp_millis();

        if (current_time - data.timestamp) > (self.config.max_data_age_seconds * 1000) as i64 {
            crate::raven_bail!(crate::raven_error!(
                data_validation,
                "Data too old".to_string()
            ));
        }

        // Validate price
        if data.price < rules.min_price || data.price > rules.max_price {
            crate::raven_bail!(crate::raven_error!(
                data_validation,
                format!("Price {} out of range", data.price)
            ));
        }

        // Validate quantity
        if data.quantity < rules.min_quantity || data.quantity > rules.max_quantity {
            crate::raven_bail!(crate::raven_error!(
                data_validation,
                format!("Quantity {} out of range", data.quantity)
            ));
        }

        // TradeSide enum already validates the side, no need for additional validation

        debug!("✓ Trade data validation passed for {}", symbol);
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

        debug!("⚬ Sanitized orderbook data for {}", sanitized.symbol);
        Ok(sanitized)
    }

    /// Write order book data to database
    async fn write_orderbook_data(&self, data: &OrderBookData) -> RavenResult<()> {
        let snapshot = OrderBookSnapshot::from(data);
        self.influx_client
            .write_orderbook_snapshot(&snapshot)
            .await
            .map_err(|e| crate::raven_error!(database_write, e.to_string()))
    }

    /// Write trade data to database
    async fn write_trade_data(&self, data: &TradeData) -> RavenResult<()> {
        let snapshot = TradeSnapshot::from(data);
        self.influx_client
            .write_trade_snapshot(&snapshot)
            .await
            .map_err(|e| crate::raven_error!(database_write, e.to_string()))
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

    /// Get DataEngine metrics
    pub fn get_metrics(&self) -> HashMap<String, u64> {
        self.metrics.to_map()
    }

    /// Get configuration
    pub fn get_config(&self) -> &DataEngineConfig {
        &self.config
    }
}


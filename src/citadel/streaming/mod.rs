// Snapshot Service - Periodic Data Capture
// "Ravens fly on schedule - capturing the realm's data every 5ms"

pub mod config;
pub mod metrics;
#[cfg(test)]
mod tests;

pub use config::SnapshotConfig;
pub use metrics::SnapshotMetrics;

use crate::citadel::storage::{HighFrequencyStorage, OrderBookSnapshot, TradeSnapshot};
use crate::database::influx_client::InfluxClient;
use crate::exchanges::types::Exchange;
use crate::subscription_manager::{SubscriptionDataType, SubscriptionManager};
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Snapshot batch for efficient database writes
#[derive(Debug, Clone)]
pub struct SnapshotBatch {
    pub orderbook_snapshots: Vec<OrderBookSnapshot>,
    pub trade_snapshots: Vec<TradeSnapshot>,
    pub timestamp: i64,
}

impl SnapshotBatch {
    pub fn new() -> Self {
        Self {
            orderbook_snapshots: Vec::new(),
            trade_snapshots: Vec::new(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.orderbook_snapshots.is_empty() && self.trade_snapshots.is_empty()
    }

    pub fn len(&self) -> usize {
        self.orderbook_snapshots.len() + self.trade_snapshots.len()
    }

    pub fn add_orderbook_snapshot(&mut self, snapshot: OrderBookSnapshot) {
        self.orderbook_snapshots.push(snapshot);
    }

    pub fn add_trade_snapshot(&mut self, snapshot: TradeSnapshot) {
        self.trade_snapshots.push(snapshot);
    }
}

impl Default for SnapshotBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// The Snapshot Service - Periodic data capture and distribution
/// "Ravens that fly on schedule, capturing the realm's data every 5ms"
pub struct SnapshotService {
    /// Configuration
    config: SnapshotConfig,
    /// High-frequency storage for atomic reads
    storage: Arc<HighFrequencyStorage>,
    /// InfluxDB client for persistence
    influx_client: Arc<InfluxClient>,
    /// Subscription manager for broadcasting
    subscription_manager: Arc<SubscriptionManager>,
    /// Service running state
    running: AtomicBool,
    /// Performance metrics
    metrics: Arc<SnapshotMetrics>,
    /// Current snapshot batch
    current_batch: Arc<RwLock<SnapshotBatch>>,
}

impl SnapshotService {
    /// Create a new snapshot service
    pub fn new(
        config: SnapshotConfig,
        storage: Arc<HighFrequencyStorage>,
        influx_client: Arc<InfluxClient>,
        subscription_manager: Arc<SubscriptionManager>,
    ) -> Self {
        info!(
            "📸 Initializing snapshot service with {:?} interval",
            config.snapshot_interval
        );

        Self {
            config,
            storage,
            influx_client,
            subscription_manager,
            running: AtomicBool::new(false),
            metrics: Arc::new(SnapshotMetrics::default()),
            current_batch: Arc::new(RwLock::new(SnapshotBatch::new())),
        }
    }

    /// Start the snapshot service
    pub async fn start(&self) -> Result<()> {
        if self
            .running
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            info!(
                "📸 Starting snapshot service - ravens will fly every {:?}",
                self.config.snapshot_interval
            );

            // Start the main snapshot loop
            let service = self.clone();
            tokio::spawn(async move {
                service.snapshot_loop().await;
            });

            // Start the batch writer if persistence is enabled
            if self.config.persistence_enabled {
                let service = self.clone();
                tokio::spawn(async move {
                    service.batch_writer_loop().await;
                });
            }

            info!("✅ Snapshot service started successfully");
        } else {
            warn!("⚠️ Snapshot service is already running");
        }

        Ok(())
    }

    /// Stop the snapshot service
    pub async fn stop(&self) -> Result<()> {
        if self
            .running
            .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            info!("📸 Stopping snapshot service...");

            // Flush any remaining batches
            if self.config.persistence_enabled {
                if let Err(e) = self.flush_current_batch().await {
                    error!("❌ Failed to flush final batch: {}", e);
                }
            }

            info!("✅ Snapshot service stopped");
        }

        Ok(())
    }

    /// Main snapshot capture loop
    async fn snapshot_loop(&self) {
        let mut interval = interval(self.config.snapshot_interval);
        info!(
            "📸 Snapshot loop started - capturing data every {:?}",
            self.config.snapshot_interval
        );

        while self.running.load(Ordering::Relaxed) {
            interval.tick().await;

            let capture_start = Instant::now();

            match self.capture_snapshots().await {
                Ok(snapshot_count) => {
                    let capture_duration = capture_start.elapsed();
                    self.metrics.update_capture_time(capture_duration);
                    self.metrics
                        .total_snapshots
                        .fetch_add(snapshot_count, Ordering::Relaxed);
                    self.metrics.last_snapshot_time.store(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        Ordering::Relaxed,
                    );

                    debug!(
                        "📸 Captured {} snapshots in {:?}",
                        snapshot_count, capture_duration
                    );

                    // Log warning if capture took longer than expected
                    if capture_duration > self.config.snapshot_interval / 2 {
                        warn!(
                            "⚠️ Snapshot capture took {:?}, which is more than half the interval ({:?})",
                            capture_duration, self.config.snapshot_interval
                        );
                    }
                }
                Err(e) => {
                    error!("❌ Snapshot capture failed: {}", e);
                    self.metrics
                        .failed_operations
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        info!("📸 Snapshot loop stopped");
    }

    /// Capture snapshots from atomic storage
    async fn capture_snapshots(&self) -> Result<u64> {
        let mut snapshot_count = 0;

        // Get all active symbols for orderbooks and trades
        let orderbook_symbols = self.storage.get_orderbook_symbols();
        let trade_symbols = self.storage.get_trade_symbols();

        let mut batch = self.current_batch.write().await;

        // Capture orderbook snapshots
        for key in orderbook_symbols {
            if let Some((exchange, symbol)) = self.parse_key(&key) {
                match self.storage.get_orderbook_snapshot(&symbol, &exchange) {
                    Some(snapshot) => {
                        // Add to batch for persistence
                        if self.config.persistence_enabled {
                            batch.add_orderbook_snapshot(snapshot.clone());
                        }

                        // Broadcast to subscribed clients
                        if self.config.broadcast_enabled {
                            if let Err(e) = self.broadcast_orderbook_snapshot(&snapshot).await {
                                warn!(
                                    "⚠️ Failed to broadcast orderbook snapshot for {}: {}",
                                    symbol, e
                                );
                            } else {
                                self.metrics
                                    .client_broadcasts
                                    .fetch_add(1, Ordering::Relaxed);
                            }
                        }

                        snapshot_count += 1;
                        self.metrics
                            .orderbook_snapshots
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        debug!("📸 No orderbook data available for symbol: {}", symbol);
                    }
                }
            }
        }

        // Capture trade snapshots
        for key in trade_symbols {
            if let Some((exchange, symbol)) = self.parse_key(&key) {
                match self.storage.get_trade_snapshot(&symbol, &exchange) {
                    Some(snapshot) => {
                        // Add to batch for persistence
                        if self.config.persistence_enabled {
                            batch.add_trade_snapshot(snapshot.clone());
                        }

                        // Broadcast to subscribed clients
                        if self.config.broadcast_enabled {
                            if let Err(e) = self.broadcast_trade_snapshot(&snapshot).await {
                                warn!(
                                    "⚠️ Failed to broadcast trade snapshot for {}: {}",
                                    symbol, e
                                );
                            } else {
                                self.metrics
                                    .client_broadcasts
                                    .fetch_add(1, Ordering::Relaxed);
                            }
                        }

                        snapshot_count += 1;
                        self.metrics.trade_snapshots.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        debug!("📸 No trade data available for symbol: {}", symbol);
                    }
                }
            }
        }

        // Check if batch should be flushed
        if batch.len() >= self.config.max_batch_size {
            drop(batch); // Release the write lock
            if let Err(e) = self.flush_current_batch().await {
                error!("❌ Failed to flush batch: {}", e);
                self.metrics
                    .failed_operations
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(snapshot_count)
    }

    /// Broadcast orderbook snapshot to subscribed gRPC clients
    async fn broadcast_orderbook_snapshot(&self, snapshot: &OrderBookSnapshot) -> Result<()> {
        // Convert to protobuf message
        let message = self.create_orderbook_message(snapshot)?;

        // Distribute to subscribed clients
        let sent_count = self
            .subscription_manager
            .distribute_message(&snapshot.symbol, SubscriptionDataType::Orderbook, message)
            .context("Failed to distribute orderbook snapshot")?;

        debug!(
            "📡 Broadcast orderbook snapshot for {} to {} clients",
            snapshot.symbol, sent_count
        );

        Ok(())
    }

    /// Broadcast trade snapshot to subscribed gRPC clients
    async fn broadcast_trade_snapshot(&self, snapshot: &TradeSnapshot) -> Result<()> {
        // Convert to protobuf message
        let message = self.create_trade_message(snapshot)?;

        // Distribute to subscribed clients
        let sent_count = self
            .subscription_manager
            .distribute_message(&snapshot.symbol, SubscriptionDataType::Trades, message)
            .context("Failed to distribute trade snapshot")?;

        debug!(
            "📡 Broadcast trade snapshot for {} to {} clients",
            snapshot.symbol, sent_count
        );

        Ok(())
    }

    /// Batch writer loop for database persistence
    async fn batch_writer_loop(&self) {
        let mut interval = interval(self.config.write_timeout);
        info!(
            "📝 Batch writer started with {:?} timeout",
            self.config.write_timeout
        );

        while self.running.load(Ordering::Relaxed) {
            interval.tick().await;

            if let Err(e) = self.flush_current_batch().await {
                error!("❌ Batch write failed: {}", e);
                self.metrics
                    .failed_operations
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        info!("📝 Batch writer stopped");
    }

    /// Flush current batch to database
    async fn flush_current_batch(&self) -> Result<()> {
        let mut batch = self.current_batch.write().await;

        if batch.is_empty() {
            return Ok(());
        }

        let write_start = Instant::now();
        let batch_size = batch.len();

        // Write orderbook snapshots
        for snapshot in &batch.orderbook_snapshots {
            self.influx_client
                .write_orderbook_snapshot(snapshot)
                .await
                .with_context(|| {
                    format!("Failed to write orderbook snapshot for {}", snapshot.symbol)
                })?;
        }

        // Write trade snapshots
        for snapshot in &batch.trade_snapshots {
            self.influx_client
                .write_trade_snapshot(snapshot)
                .await
                .with_context(|| {
                    format!("Failed to write trade snapshot for {}", snapshot.symbol)
                })?;
        }

        let write_duration = write_start.elapsed();
        self.metrics.update_write_time(write_duration);
        self.metrics
            .database_writes
            .fetch_add(batch_size as u64, Ordering::Relaxed);

        debug!(
            "📝 Flushed batch of {} snapshots to database in {:?}",
            batch_size, write_duration
        );

        // Clear the batch
        *batch = SnapshotBatch::new();

        Ok(())
    }

    /// Create protobuf message from orderbook snapshot
    fn create_orderbook_message(
        &self,
        snapshot: &OrderBookSnapshot,
    ) -> Result<crate::proto::MarketDataMessage> {
        use crate::proto::{MarketDataMessage, OrderBookSnapshot as ProtoOrderBook, PriceLevel};

        let proto_snapshot = ProtoOrderBook {
            symbol: snapshot.symbol.clone(),
            timestamp: snapshot.timestamp,
            bids: vec![PriceLevel {
                price: snapshot.best_bid_price,
                quantity: snapshot.best_bid_quantity,
            }],
            asks: vec![PriceLevel {
                price: snapshot.best_ask_price,
                quantity: snapshot.best_ask_quantity,
            }],
            sequence: snapshot.sequence as i64,
        };

        Ok(MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Orderbook(
                proto_snapshot,
            )),
        })
    }

    /// Create protobuf message from trade snapshot
    fn create_trade_message(
        &self,
        snapshot: &TradeSnapshot,
    ) -> Result<crate::proto::MarketDataMessage> {
        use crate::proto::{MarketDataMessage, Trade as ProtoTrade};

        let proto_trade = ProtoTrade {
            symbol: snapshot.symbol.clone(),
            timestamp: snapshot.timestamp,
            price: snapshot.price,
            quantity: snapshot.quantity,
            side: snapshot.side.to_string(),
            trade_id: snapshot.trade_id.to_string(),
        };

        Ok(MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Trade(proto_trade)),
        })
    }

    /// Get snapshot service metrics
    pub fn get_metrics(&self) -> std::collections::HashMap<String, u64> {
        self.metrics.to_map()
    }

    /// Get snapshot service status
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get current configuration
    pub fn get_config(&self) -> &SnapshotConfig {
        &self.config
    }

    /// Update configuration (requires restart to take effect)
    pub fn update_config(&mut self, config: SnapshotConfig) {
        self.config = config;
        info!("📸 Snapshot service configuration updated");
    }

    /// Force flush current batch (for testing or manual operations)
    pub async fn force_flush(&self) -> Result<()> {
        self.flush_current_batch().await
    }

    /// Get current batch size
    pub async fn get_current_batch_size(&self) -> usize {
        self.current_batch.read().await.len()
    }

    /// Parse exchange and symbol from storage key (format: "exchange:symbol")
    fn parse_key(&self, key: &str) -> Option<(Exchange, String)> {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let exchange = match parts[0] {
            "binance_spot" => Exchange::BinanceSpot,
            "binance_futures" => Exchange::BinanceFutures,
            // "coinbase" => Exchange::Coinbase,
            // "kraken" => Exchange::Kraken,
            // "bybit" => Exchange::Bybit,
            // "okx" => Exchange::OKX,
            // "deribit" => Exchange::Deribit,
            _ => return None,
        };

        Some((exchange, parts[1].to_string()))
    }
}

impl Clone for SnapshotService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: Arc::clone(&self.storage),
            influx_client: Arc::clone(&self.influx_client),
            subscription_manager: Arc::clone(&self.subscription_manager),
            running: AtomicBool::new(self.running.load(Ordering::Relaxed)),
            metrics: Arc::clone(&self.metrics),
            current_batch: Arc::clone(&self.current_batch),
        }
    }
}

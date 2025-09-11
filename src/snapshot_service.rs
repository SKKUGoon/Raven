// Snapshot Service - Periodic Data Capture
// "Ravens fly on schedule - capturing the realm's data every 5ms"

use crate::database::influx_client::InfluxClient;
use crate::subscription_manager::{SubscriptionDataType, SubscriptionManager};
use crate::types::{HighFrequencyStorage, OrderBookSnapshot, TradeSnapshot};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Configuration for the snapshot service
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Interval between snapshots (default: 5ms)
    pub snapshot_interval: Duration,
    /// Maximum number of snapshots to batch before writing to database
    pub max_batch_size: usize,
    /// Timeout for database writes
    pub write_timeout: Duration,
    /// Whether to broadcast snapshots to gRPC clients
    pub broadcast_enabled: bool,
    /// Whether to persist snapshots to database
    pub persistence_enabled: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_interval: Duration::from_millis(5),
            max_batch_size: 1000,
            write_timeout: Duration::from_millis(100),
            broadcast_enabled: true,
            persistence_enabled: true,
        }
    }
}

/// Performance metrics for the snapshot service
#[derive(Debug, Default)]
pub struct SnapshotMetrics {
    /// Total number of snapshots captured
    pub total_snapshots: AtomicU64,
    /// Total number of orderbook snapshots
    pub orderbook_snapshots: AtomicU64,
    /// Total number of trade snapshots
    pub trade_snapshots: AtomicU64,
    /// Total number of snapshots written to database
    pub database_writes: AtomicU64,
    /// Total number of snapshots broadcast to clients
    pub client_broadcasts: AtomicU64,
    /// Total number of failed operations
    pub failed_operations: AtomicU64,
    /// Last snapshot timestamp
    pub last_snapshot_time: AtomicU64,
    /// Average snapshot capture time in nanoseconds
    pub avg_capture_time_ns: AtomicU64,
    /// Average database write time in nanoseconds
    pub avg_write_time_ns: AtomicU64,
}

impl SnapshotMetrics {
    /// Get metrics as a HashMap for monitoring
    pub fn to_map(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();
        metrics.insert(
            "total_snapshots".to_string(),
            self.total_snapshots.load(Ordering::Relaxed),
        );
        metrics.insert(
            "orderbook_snapshots".to_string(),
            self.orderbook_snapshots.load(Ordering::Relaxed),
        );
        metrics.insert(
            "trade_snapshots".to_string(),
            self.trade_snapshots.load(Ordering::Relaxed),
        );
        metrics.insert(
            "database_writes".to_string(),
            self.database_writes.load(Ordering::Relaxed),
        );
        metrics.insert(
            "client_broadcasts".to_string(),
            self.client_broadcasts.load(Ordering::Relaxed),
        );
        metrics.insert(
            "failed_operations".to_string(),
            self.failed_operations.load(Ordering::Relaxed),
        );
        metrics.insert(
            "last_snapshot_time".to_string(),
            self.last_snapshot_time.load(Ordering::Relaxed),
        );
        metrics.insert(
            "avg_capture_time_ns".to_string(),
            self.avg_capture_time_ns.load(Ordering::Relaxed),
        );
        metrics.insert(
            "avg_write_time_ns".to_string(),
            self.avg_write_time_ns.load(Ordering::Relaxed),
        );
        metrics
    }

    /// Update average capture time
    pub fn update_capture_time(&self, duration: Duration) {
        let new_time = duration.as_nanos() as u64;
        let current_avg = self.avg_capture_time_ns.load(Ordering::Relaxed);
        // Simple moving average approximation
        let new_avg = if current_avg == 0 {
            new_time
        } else {
            (current_avg * 9 + new_time) / 10
        };
        self.avg_capture_time_ns.store(new_avg, Ordering::Relaxed);
    }

    /// Update average write time
    pub fn update_write_time(&self, duration: Duration) {
        let new_time = duration.as_nanos() as u64;
        let current_avg = self.avg_write_time_ns.load(Ordering::Relaxed);
        // Simple moving average approximation
        let new_avg = if current_avg == 0 {
            new_time
        } else {
            (current_avg * 9 + new_time) / 10
        };
        self.avg_write_time_ns.store(new_avg, Ordering::Relaxed);
    }
}

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
        for symbol in orderbook_symbols {
            match self.storage.get_orderbook_snapshot(&symbol) {
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

        // Capture trade snapshots
        for symbol in trade_symbols {
            match self.storage.get_trade_snapshot(&symbol) {
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
            side: snapshot.side.clone(),
            trade_id: snapshot.trade_id.to_string(),
        };

        Ok(MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Trade(proto_trade)),
        })
    }

    /// Get snapshot service metrics
    pub fn get_metrics(&self) -> HashMap<String, u64> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::influx_client::InfluxConfig;
    use crate::types::{OrderBookData, TradeData};

    #[tokio::test]
    async fn test_snapshot_service_creation() {
        let config = SnapshotConfig::default();
        let storage = Arc::new(HighFrequencyStorage::new());
        let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
        let subscription_manager = Arc::new(SubscriptionManager::new());

        let service = SnapshotService::new(config, storage, influx_client, subscription_manager);

        assert!(!service.is_running());
        assert_eq!(service.get_current_batch_size().await, 0);
    }

    #[tokio::test]
    async fn test_snapshot_batch() {
        let mut batch = SnapshotBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);

        let orderbook_snapshot = OrderBookSnapshot {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            best_bid_price: 45000.0,
            best_bid_quantity: 1.5,
            best_ask_price: 45001.0,
            best_ask_quantity: 1.2,
            sequence: 12345,
        };

        batch.add_orderbook_snapshot(orderbook_snapshot);
        assert!(!batch.is_empty());
        assert_eq!(batch.len(), 1);

        let trade_snapshot = TradeSnapshot {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: 123456,
        };

        batch.add_trade_snapshot(trade_snapshot);
        assert_eq!(batch.len(), 2);
    }

    #[tokio::test]
    async fn test_snapshot_metrics() {
        let metrics = SnapshotMetrics::default();

        metrics.total_snapshots.store(100, Ordering::Relaxed);
        metrics.orderbook_snapshots.store(60, Ordering::Relaxed);
        metrics.trade_snapshots.store(40, Ordering::Relaxed);

        let metrics_map = metrics.to_map();
        assert_eq!(metrics_map.get("total_snapshots"), Some(&100));
        assert_eq!(metrics_map.get("orderbook_snapshots"), Some(&60));
        assert_eq!(metrics_map.get("trade_snapshots"), Some(&40));
    }

    #[tokio::test]
    async fn test_capture_snapshots_with_data() {
        let config = SnapshotConfig::default();
        let storage = Arc::new(HighFrequencyStorage::new());
        let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
        let subscription_manager = Arc::new(SubscriptionManager::new());

        // Add some test data to storage
        let orderbook_data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![(45001.0, 1.2)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        let trade_data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "trade123".to_string(),
            exchange: "binance".to_string(),
        };

        storage.update_orderbook(&orderbook_data);
        storage.update_trade(&trade_data);

        let mut service_config = config;
        service_config.persistence_enabled = false; // Disable persistence for test
        service_config.broadcast_enabled = false; // Disable broadcast for test

        let service =
            SnapshotService::new(service_config, storage, influx_client, subscription_manager);

        // Capture snapshots
        let snapshot_count = service.capture_snapshots().await.unwrap();
        assert_eq!(snapshot_count, 2); // 1 orderbook + 1 trade

        let metrics = service.get_metrics();
        assert_eq!(metrics.get("orderbook_snapshots"), Some(&1));
        assert_eq!(metrics.get("trade_snapshots"), Some(&1));
    }

    #[test]
    fn test_snapshot_config() {
        let config = SnapshotConfig::default();
        assert_eq!(config.snapshot_interval, Duration::from_millis(5));
        assert_eq!(config.max_batch_size, 1000);
        assert!(config.broadcast_enabled);
        assert!(config.persistence_enabled);

        let custom_config = SnapshotConfig {
            snapshot_interval: Duration::from_millis(10),
            max_batch_size: 500,
            write_timeout: Duration::from_millis(50),
            broadcast_enabled: false,
            persistence_enabled: true,
        };

        assert_eq!(custom_config.snapshot_interval, Duration::from_millis(10));
        assert_eq!(custom_config.max_batch_size, 500);
        assert!(!custom_config.broadcast_enabled);
        assert!(custom_config.persistence_enabled);
    }
}

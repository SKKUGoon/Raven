use crate::database::influx_client::InfluxConfig;
use crate::database::InfluxClient;
use crate::exchanges::Exchange;
use crate::snapshot_service::{SnapshotBatch, SnapshotConfig, SnapshotMetrics, SnapshotService};
use crate::subscription_manager::SubscriptionManager;
use crate::types::{
    HighFrequencyStorage, OrderBookData, OrderBookSnapshot, TradeData, TradeSide, TradeSnapshot,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

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
        side: TradeSide::Buy,
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
        exchange: Exchange::BinanceSpot,
    };

    let trade_data = TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: "trade123".to_string(),
        exchange: Exchange::BinanceSpot,
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

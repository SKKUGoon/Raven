use raven::database::influx_client::{InfluxClient, InfluxConfig};
use raven::exchanges::types::Exchange;
use raven::snapshot_service::{SnapshotBatch, SnapshotConfig, SnapshotMetrics, SnapshotService};
use raven::subscription_manager::SubscriptionManager;
use raven::types::{
    HighFrequencyStorage, OrderBookLevel, OrderBookSnapshot, TradeSide, TradeSnapshot,
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
        exchange: Exchange::BinanceSpot,
        timestamp: 1640995200000,
        best_bid_price: 45000.0,
        best_bid_quantity: 1.5,
        best_ask_price: 45001.0,
        best_ask_quantity: 1.2,
        sequence: 12345,
        bid_levels: vec![OrderBookLevel {
            price: 45000.0,
            quantity: 1.5,
        }],
        ask_levels: vec![OrderBookLevel {
            price: 45001.0,
            quantity: 1.2,
        }],
    };

    batch.add_orderbook_snapshot(orderbook_snapshot);
    assert!(!batch.is_empty());
    assert_eq!(batch.len(), 1);

    let trade_snapshot = TradeSnapshot {
        symbol: "BTCUSDT".to_string(),
        exchange: Exchange::BinanceSpot,
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

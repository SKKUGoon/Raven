use crate::database::{DatabaseDeadLetterHelper, EnhancedInfluxClient, InfluxClient, InfluxConfig};
use crate::dead_letter_queue::{DeadLetterQueue, DeadLetterQueueConfig};
use crate::types::{OrderBookSnapshot, TradeSide, TradeSnapshot};
use std::sync::Arc;

#[tokio::test]
async fn test_database_dead_letter_helper_orderbook() {
    let snapshot = OrderBookSnapshot {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        best_bid_price: 45000.0,
        best_bid_quantity: 1.5,
        best_ask_price: 45001.0,
        best_ask_quantity: 1.2,
        sequence: 12345,
    };

    let entry =
        DatabaseDeadLetterHelper::create_orderbook_entry(&snapshot, "Test error".to_string())
            .unwrap();

    assert_eq!(entry.operation_type, "influx_write");
    assert_eq!(
        entry.metadata.get("subtype"),
        Some(&"orderbook_snapshot".to_string())
    );
    assert_eq!(entry.metadata.get("symbol"), Some(&"BTCUSDT".to_string()));
}

#[tokio::test]
async fn test_database_dead_letter_helper_trade() {
    let snapshot = TradeSnapshot {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: 67890,
    };

    let entry =
        DatabaseDeadLetterHelper::create_trade_entry(&snapshot, "Test error".to_string()).unwrap();

    assert_eq!(entry.operation_type, "influx_write");
    assert_eq!(
        entry.metadata.get("subtype"),
        Some(&"trade_snapshot".to_string())
    );
    assert_eq!(entry.metadata.get("symbol"), Some(&"BTCUSDT".to_string()));
}

#[tokio::test]
async fn test_enhanced_influx_client_creation() {
    let influx_config = InfluxConfig::default();
    let influx_client = Arc::new(InfluxClient::new(influx_config));

    let dlq_config = DeadLetterQueueConfig::default();
    let dead_letter_queue = Arc::new(DeadLetterQueue::new(dlq_config));

    let enhanced_client = EnhancedInfluxClient::new(influx_client, dead_letter_queue);

    // Test that we can access both components
    assert!(enhanced_client
        .client()
        .get_pool_status()
        .await
        .contains_key("total_connections"));
    assert_eq!(
        enhanced_client
            .dead_letter_queue()
            .get_statistics()
            .await
            .total_entries,
        0
    );
}

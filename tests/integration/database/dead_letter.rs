use raven::data_engine::storage::{OrderBookLevel, OrderBookSnapshot, TradeSide, TradeSnapshot};
use raven::database::dead_letter_queue::DeadLetterEntry;
use raven::database::{DatabaseDeadLetterHelper, InfluxClient, InfluxConfig};
use raven::exchanges::types::Exchange;

#[tokio::test]
async fn test_database_dead_letter_helper_orderbook() {
    let snapshot = OrderBookSnapshot {
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

    let entry =
        DatabaseDeadLetterHelper::create_orderbook_entry(&snapshot, "Test error".into()).unwrap();

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
        exchange: Exchange::BinanceSpot,
        timestamp: 1640995200000,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: 67890,
    };

    let entry =
        DatabaseDeadLetterHelper::create_trade_entry(&snapshot, "Test error".into()).unwrap();

    assert_eq!(entry.operation_type, "influx_write");
    assert_eq!(
        entry.metadata.get("subtype"),
        Some(&"trade_snapshot".to_string())
    );
    assert_eq!(entry.metadata.get("symbol"), Some(&"BTCUSDT".to_string()));
}

#[tokio::test]
async fn test_dead_letter_queue() {
    let client = InfluxClient::new(InfluxConfig::default());

    client
        .add_to_dead_letter_queue("test query".into(), "test error".into())
        .await;

    let status = client.get_pool_status().await;
    assert_eq!(status.get("dead_letter_queue_size").unwrap(), &1);
}

#[tokio::test]
async fn test_process_dead_letter_queue() {
    let client = InfluxClient::new(InfluxConfig::default());

    client
        .add_to_dead_letter_queue("test query 1".into(), "error 1".into())
        .await;
    client
        .add_to_dead_letter_queue("test query 2".into(), "error 2".into())
        .await;

    let status = client.get_pool_status().await;
    assert_eq!(status.get("dead_letter_queue_size").unwrap(), &2);

    let processed = client.process_dead_letter_queue().await.unwrap();
    assert_eq!(processed, 2);

    let status = client.get_pool_status().await;
    assert_eq!(status.get("dead_letter_queue_size").unwrap(), &0);
}

#[test]
fn test_dead_letter_entry_creation() {
    let entry = DeadLetterEntry::new("write", "test data", "test error", 3);

    assert_eq!(entry.operation_type, "write");
    assert_eq!(entry.data, "test data");
    assert_eq!(entry.retry_count, 0);
    assert_eq!(entry.max_retries, 3);
    assert_eq!(entry.last_error, "test error");
}

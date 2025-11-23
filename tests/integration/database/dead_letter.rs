use raven::server::data_engine::storage::{OrderBookLevel, OrderBookSnapshot, TradeSide, TradeSnapshot};
use raven::common::db::dead_letter_queue::{DeadLetterEntry, DeadLetterQueue, DeadLetterQueueConfig, RetryHandler};
use raven::common::db::DatabaseDeadLetterHelper;
use raven::server::exchanges::types::Exchange;
use raven::common::error::RavenResult;

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
    let config = DeadLetterQueueConfig::default();
    let queue = DeadLetterQueue::new(config);

    let entry = DeadLetterEntry::new("test", "test data", "test error", 3);
    queue.add_entry(entry).await.unwrap();

    let stats = queue.get_statistics().await;
    assert_eq!(stats.total_entries, 1);
}

struct MockRetryHandler;

#[async_trait::async_trait]
impl RetryHandler for MockRetryHandler {
    async fn retry_operation(&self, _entry: &DeadLetterEntry) -> RavenResult<()> {
        Ok(())
    }

    fn operation_type(&self) -> &str {
        "test"
    }
}

#[tokio::test]
async fn test_process_dead_letter_queue() {
    let config = DeadLetterQueueConfig {
        processing_interval_ms: 10, // Faster for test
        ..Default::default()
    };
    let queue = DeadLetterQueue::new(config);

    // Register handler
    let handler = Box::new(MockRetryHandler);
    queue.register_retry_handler(handler).await;

    // Add entries
    let entry1 = DeadLetterEntry::new("test", "data1", "error1", 3);
    let mut entry2 = DeadLetterEntry::new("test", "data2", "error2", 3);
    // Make entry2 ready for retry immediately
    entry2.next_retry_timestamp = 0;

    // Make entry1 NOT ready for retry
    let future_time = raven::common::current_timestamp_millis() + 10000;
    let mut entry1 = entry1;
    entry1.next_retry_timestamp = future_time;

    queue.add_entry(entry1).await.unwrap();
    queue.add_entry(entry2).await.unwrap();

    let stats = queue.get_statistics().await;
    assert_eq!(stats.total_entries, 2);

    // Start processing
    queue.start_processing().await.unwrap();

    // Wait for processing
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    queue.stop_processing();

    let stats = queue.get_statistics().await;
    // entry2 should be processed and removed, entry1 should remain
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.successful_retries, 1);
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

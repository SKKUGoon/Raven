// DataEngine Tests - Project Raven

use raven::server::data_engine::storage::{OrderBookData, TradeData, TradeSide};
use raven::server::data_engine::{DataEngine, DataEngineConfig, ValidationRules};
use raven::current_timestamp_millis;
use raven::server::database::influx_client::{InfluxClient, InfluxConfig};
use raven::server::database::DeadLetterQueue;
use raven::server::exchanges::types::Exchange;
use raven::server::subscription_manager::SubscriptionManager;
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn create_test_data_engine() -> DataEngine {
    let influx_config = InfluxConfig::default();
    let influx_client = Arc::new(InfluxClient::new(influx_config));
    let subscription_manager = Arc::new(SubscriptionManager::new());
    let data_engine_config = DataEngineConfig::default();
    let dead_letter_queue = Arc::new(DeadLetterQueue::new(Default::default()));

    DataEngine::new(data_engine_config, influx_client, subscription_manager, dead_letter_queue)
}

fn create_test_orderbook_data() -> OrderBookData {
    OrderBookData {
        symbol: "BTCUSDT".to_string(),
        timestamp: current_timestamp_millis(),
        bids: vec![(45000.0, 1.5), (44999.0, 2.0)],
        asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
        sequence: 12345,
        exchange: Exchange::BinanceSpot,
    }
}

fn create_test_trade_data() -> TradeData {
    TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: current_timestamp_millis(),
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: "test_trade_123".to_string(),
        exchange: Exchange::BinanceSpot,
    }
}

#[tokio::test]
async fn test_data_engine_creation() {
    let data_engine = create_test_data_engine();
    let metrics = data_engine.get_metrics();

    assert_eq!(metrics.get("total_ingested").unwrap(), &0);
    assert_eq!(metrics.get("total_validated").unwrap(), &0);
    assert_eq!(metrics.get("total_written").unwrap(), &0);
}

#[tokio::test]
async fn test_orderbook_validation() {
    let data_engine = create_test_data_engine();
    let data = create_test_orderbook_data();

    let result = data_engine.validate_orderbook_data("BTCUSDT", &data).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_trade_validation() {
    let data_engine = create_test_data_engine();
    let data = create_test_trade_data();

    let result = data_engine.validate_trade_data("BTCUSDT", &data).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_invalid_orderbook_validation() {
    let data_engine = create_test_data_engine();
    let mut data = create_test_orderbook_data();

    // Test invalid price
    data.bids = vec![(-1.0, 1.0)];
    let result = data_engine.validate_orderbook_data("BTCUSDT", &data).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_trade_validation() {
    let data_engine = create_test_data_engine();
    let mut data = create_test_trade_data();

    // Test invalid price instead of invalid side
    data.price = -1.0;
    let result = data_engine.validate_trade_data("BTCUSDT", &data).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_data_sanitization() {
    let data_engine = create_test_data_engine();
    let mut data = create_test_orderbook_data();

    // Add whitespace and lowercase
    data.symbol = " btcusdt ".to_string();
    data.exchange = Exchange::BinanceSpot;

    let sanitized = data_engine.sanitize_orderbook_data(&data).await.unwrap();
    assert_eq!(sanitized.symbol, "BTCUSDT");
    assert_eq!(sanitized.exchange, Exchange::BinanceSpot);
}

#[tokio::test]
async fn test_validation_rules_update() {
    let data_engine = create_test_data_engine();

    let rules = ValidationRules {
        min_price: 1000.0,
        max_price: 50000.0,
        ..ValidationRules::default()
    };

    data_engine
        .update_validation_rules(rules.clone())
        .await
        .unwrap();
    let retrieved_rules = data_engine.get_validation_rules().await;

    assert_eq!(retrieved_rules.min_price, 1000.0);
    assert_eq!(retrieved_rules.max_price, 50000.0);
}

#[tokio::test]
async fn test_dead_letter_queue_status() {
    let data_engine = create_test_data_engine();

    // Add an entry to dead letter queue
    data_engine
        .add_to_dead_letter_queue(
            "test".to_string(),
            "test_data".to_string(),
            "test_error".to_string(),
        )
        .await;

    let status = data_engine.get_dead_letter_queue_status().await;
    assert_eq!(status.get("total_entries").unwrap(), &1);
    assert_eq!(status.get("test_entries").unwrap(), &1);
}

#[tokio::test]
async fn test_metrics_tracking() {
    let data_engine = create_test_data_engine();

    // Simulate some operations
    data_engine
        .metrics
        .total_ingested
        .fetch_add(5, Ordering::Relaxed);
    data_engine
        .metrics
        .total_validated
        .fetch_add(4, Ordering::Relaxed);
    data_engine
        .metrics
        .total_written
        .fetch_add(3, Ordering::Relaxed);
    data_engine.metrics.total_failed.fetch_add(1, Ordering::Relaxed);

    let metrics = data_engine.get_metrics();
    assert_eq!(metrics.get("total_ingested").unwrap(), &5);
    assert_eq!(metrics.get("total_validated").unwrap(), &4);
    assert_eq!(metrics.get("total_written").unwrap(), &3);
    assert_eq!(metrics.get("total_failed").unwrap(), &1);
}

#[tokio::test]
async fn test_data_engine_config_validation_rules_wiring() {
    let mut config = DataEngineConfig::default();
    config.max_price = 5000.0;
    config.max_price_deviation = 20.0;

    let influx_config = InfluxConfig::default();
    let influx_client = Arc::new(InfluxClient::new(influx_config));
    let subscription_manager = Arc::new(SubscriptionManager::new());
    let dead_letter_queue = Arc::new(DeadLetterQueue::new(Default::default()));

    let data_engine = DataEngine::new(config, influx_client, subscription_manager, dead_letter_queue);
    let rules = data_engine.get_validation_rules().await;

    assert_eq!(rules.max_price, 5000.0);
    assert_eq!(rules.max_price_deviation, 20.0);
}

#[tokio::test]
async fn test_price_quantity_rounding() {
    let data_engine = create_test_data_engine();

    let price = 45000.123456789;
    let rounded_price = data_engine.round_price(price);
    assert_eq!(rounded_price, 45000.12345679);

    let quantity = 1.987654321;
    let rounded_quantity = data_engine.round_quantity(quantity);
    assert_eq!(rounded_quantity, 1.98765432);
}


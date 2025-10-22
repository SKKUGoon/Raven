// Citadel Tests - Project Raven
// "Testing the fortress that guards our data"

use raven::citadel::storage::{OrderBookData, TradeData, TradeSide};
use raven::citadel::{Citadel, CitadelConfig, ValidationRules};
use raven::database::influx_client::{InfluxClient, InfluxConfig};
use raven::exchanges::types::Exchange;
use raven::subscription_manager::SubscriptionManager;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn create_test_citadel() -> Citadel {
    let influx_config = InfluxConfig::default();
    let influx_client = Arc::new(InfluxClient::new(influx_config));
    let subscription_manager = Arc::new(SubscriptionManager::new());
    let citadel_config = CitadelConfig::default();

    Citadel::new(citadel_config, influx_client, subscription_manager)
}

fn create_test_orderbook_data() -> OrderBookData {
    OrderBookData {
        symbol: "BTCUSDT".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        bids: vec![(45000.0, 1.5), (44999.0, 2.0)],
        asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
        sequence: 12345,
        exchange: Exchange::BinanceSpot,
    }
}

fn create_test_trade_data() -> TradeData {
    TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: "test_trade_123".to_string(),
        exchange: Exchange::BinanceSpot,
    }
}

#[tokio::test]
async fn test_citadel_creation() {
    let citadel = create_test_citadel();
    let metrics = citadel.get_metrics();

    assert_eq!(metrics.get("total_ingested").unwrap(), &0);
    assert_eq!(metrics.get("total_validated").unwrap(), &0);
    assert_eq!(metrics.get("total_written").unwrap(), &0);
}

#[tokio::test]
async fn test_orderbook_validation() {
    let citadel = create_test_citadel();
    let data = create_test_orderbook_data();

    let result = citadel.validate_orderbook_data("BTCUSDT", &data).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_trade_validation() {
    let citadel = create_test_citadel();
    let data = create_test_trade_data();

    let result = citadel.validate_trade_data("BTCUSDT", &data).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_invalid_orderbook_validation() {
    let citadel = create_test_citadel();
    let mut data = create_test_orderbook_data();

    // Test invalid price
    data.bids = vec![(-1.0, 1.0)];
    let result = citadel.validate_orderbook_data("BTCUSDT", &data).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_trade_validation() {
    let citadel = create_test_citadel();
    let mut data = create_test_trade_data();

    // Test invalid price instead of invalid side
    data.price = -1.0;
    let result = citadel.validate_trade_data("BTCUSDT", &data).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_data_sanitization() {
    let citadel = create_test_citadel();
    let mut data = create_test_orderbook_data();

    // Add whitespace and lowercase
    data.symbol = " btcusdt ".to_string();
    data.exchange = Exchange::BinanceSpot;

    let sanitized = citadel.sanitize_orderbook_data(&data).await.unwrap();
    assert_eq!(sanitized.symbol, "BTCUSDT");
    assert_eq!(sanitized.exchange, Exchange::BinanceSpot);
}

#[tokio::test]
async fn test_validation_rules_update() {
    let citadel = create_test_citadel();

    let rules = ValidationRules {
        min_price: 1000.0,
        max_price: 50000.0,
        ..ValidationRules::default()
    };

    citadel
        .update_validation_rules(rules.clone())
        .await
        .unwrap();
    let retrieved_rules = citadel.get_validation_rules().await;

    assert_eq!(retrieved_rules.min_price, 1000.0);
    assert_eq!(retrieved_rules.max_price, 50000.0);
}

#[tokio::test]
async fn test_dead_letter_queue_status() {
    let citadel = create_test_citadel();

    // Add an entry to dead letter queue
    citadel
        .add_to_dead_letter_queue(
            "test".to_string(),
            "test_data".to_string(),
            "test_error".to_string(),
        )
        .await;

    let status = citadel.get_dead_letter_queue_status().await;
    assert_eq!(status.get("total_entries").unwrap(), &1);
    assert_eq!(status.get("test_entries").unwrap(), &1);
}

#[tokio::test]
async fn test_metrics_tracking() {
    let citadel = create_test_citadel();

    // Simulate some operations
    citadel
        .metrics
        .total_ingested
        .fetch_add(5, Ordering::Relaxed);
    citadel
        .metrics
        .total_validated
        .fetch_add(4, Ordering::Relaxed);
    citadel
        .metrics
        .total_written
        .fetch_add(3, Ordering::Relaxed);
    citadel.metrics.total_failed.fetch_add(1, Ordering::Relaxed);

    let metrics = citadel.get_metrics();
    assert_eq!(metrics.get("total_ingested").unwrap(), &5);
    assert_eq!(metrics.get("total_validated").unwrap(), &4);
    assert_eq!(metrics.get("total_written").unwrap(), &3);
    assert_eq!(metrics.get("total_failed").unwrap(), &1);
}

#[tokio::test]
async fn test_price_quantity_rounding() {
    let citadel = create_test_citadel();

    let price = 45000.123456789;
    let rounded_price = citadel.round_price(price);
    assert_eq!(rounded_price, 45000.12345679);

    let quantity = 1.987654321;
    let rounded_quantity = citadel.round_quantity(quantity);
    assert_eq!(rounded_quantity, 1.98765432);
}

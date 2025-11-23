use raven::current_timestamp_millis;
use raven::server::data_engine::storage::{
    CandleData, FundingRateData, OrderBookLevel, OrderBookSnapshot, TradeSide, TradeSnapshot,
};
use raven::common::db::dead_letter_queue::{DeadLetterQueue, DeadLetterQueueConfig};
use raven::common::db::influx_client::{create_orderbook_datapoint, InfluxClient, InfluxConfig};
use raven::common::db::EnhancedInfluxClient;
use raven::server::exchanges::types::Exchange;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_enhanced_influx_client_creation() {
    let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
    let dead_letter_queue = Arc::new(DeadLetterQueue::new(DeadLetterQueueConfig::default()));

    let enhanced_client = EnhancedInfluxClient::new(influx_client, dead_letter_queue);

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

#[tokio::test]
async fn test_influx_client_creation() {
    let client = InfluxClient::new(InfluxConfig::default());

    assert_eq!(client.config.bucket, "crypto");
    assert_eq!(client.config.pool_size, 10);
    assert!(!client.health_check_running.load(Ordering::Relaxed));
}

#[tokio::test]
async fn test_write_operations() {
    let client = InfluxClient::new(InfluxConfig::default());

    let snapshot = OrderBookSnapshot {
        symbol: "BTCUSDT".to_string(),
        exchange: Exchange::BinanceSpot,
        timestamp: current_timestamp_millis(),
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

    assert!(client.write_orderbook_snapshot(&snapshot).await.is_err());

    let trade_snapshot = TradeSnapshot {
        symbol: "BTCUSDT".to_string(),
        exchange: Exchange::BinanceSpot,
        timestamp: current_timestamp_millis(),
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: 123456,
    };

    assert!(client.write_trade_snapshot(&trade_snapshot).await.is_err());

    let candle = CandleData {
        symbol: "BTCUSDT".to_string(),
        timestamp: current_timestamp_millis(),
        open: 45000.0,
        high: 45100.0,
        low: 44900.0,
        close: 45050.0,
        volume: 150.5,
        interval: "1m".to_string(),
        exchange: Exchange::BinanceSpot,
    };

    assert!(client.write_candle(&candle).await.is_err());

    let funding = FundingRateData {
        symbol: "BTCUSDT".to_string(),
        timestamp: current_timestamp_millis(),
        rate: 0.0001,
        next_funding_time: current_timestamp_millis() + 28_800_000,
        exchange: Exchange::BinanceSpot,
    };

    assert!(client.write_funding_rate(&funding).await.is_err());

    let balances = vec![
        ("BTC".to_string(), 1.5, 0.1),
        ("USDT".to_string(), 10000.0, 500.0),
    ];

    assert!(client
        .write_wallet_update("user123", &balances, current_timestamp_millis(),)
        .await
        .is_err());
}

#[tokio::test]
async fn test_config_defaults() {
    let config = InfluxConfig::default();

    assert_eq!(config.url, "http://localhost:8086");
    assert_eq!(config.bucket, "crypto");
    assert_eq!(config.org, "raven");
    assert_eq!(config.pool_size, 10);
    assert_eq!(config.retry_attempts, 3);
    assert_eq!(config.batch_size, 1000);
    assert!(config.token.is_none());
}

#[tokio::test]
async fn test_batch_write_empty() {
    let client = InfluxClient::new(InfluxConfig::default());
    assert!(client.write_batch(vec![]).await.is_ok());
}

#[tokio::test]
async fn test_batch_write_with_datapoints() {
    let client = InfluxClient::new(InfluxConfig::default());

    let snapshot1 = OrderBookSnapshot {
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

    let snapshot2 = OrderBookSnapshot {
        symbol: "ETHUSDT".to_string(),
        exchange: Exchange::BinanceSpot,
        timestamp: 1640995200001,
        best_bid_price: 3500.0,
        best_bid_quantity: 2.5,
        best_ask_price: 3501.0,
        best_ask_quantity: 2.2,
        sequence: 12346,
        bid_levels: vec![OrderBookLevel {
            price: 3500.0,
            quantity: 2.5,
        }],
        ask_levels: vec![OrderBookLevel {
            price: 3501.0,
            quantity: 2.2,
        }],
    };

    let datapoints = vec![
        create_orderbook_datapoint(&snapshot1).unwrap(),
        create_orderbook_datapoint(&snapshot2).unwrap(),
    ];

    assert!(client.write_batch(datapoints).await.is_err());
}

#[tokio::test]
async fn test_pool_status() {
    let client = InfluxClient::new(InfluxConfig::default());
    let status = client.get_pool_status().await;

    assert!(status.contains_key("total_connections"));
    assert!(status.contains_key("healthy_connections"));
    assert!(status.contains_key("unhealthy_connections"));
    assert!(status.contains_key("circuit_breaker_state"));
    assert!(status.contains_key("health_monitoring_active"));

    assert_eq!(status.get("total_connections").unwrap(), &0);
    assert_eq!(status.get("healthy_connections").unwrap(), &0);
}

#[tokio::test]
async fn test_query_historical_data_placeholder() {
    let client = InfluxClient::new(InfluxConfig::default());

    let result = client
        .query_historical_data(
            "orderbook",
            "BTCUSDT",
            1640995200000,
            1640995300000,
            Some(10),
        )
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_influx_config_custom() {
    let config = InfluxConfig {
        url: "http://custom:8086".to_string(),
        bucket: "custom_bucket".to_string(),
        org: "custom_org".to_string(),
        token: Some("custom_token".to_string()),
        pool_size: 5,
        timeout: Duration::from_secs(30),
        retry_attempts: 5,
        retry_delay: Duration::from_millis(200),
        batch_size: 500,
        flush_interval: Duration::from_millis(10),
    };

    assert_eq!(config.url, "http://custom:8086");
    assert_eq!(config.bucket, "custom_bucket");
    assert_eq!(config.org, "custom_org");
    assert_eq!(config.token, Some("custom_token".to_string()));
    assert_eq!(config.pool_size, 5);
    assert_eq!(config.retry_attempts, 5);
    assert_eq!(config.batch_size, 500);
}

use super::*;
use crate::types::{CandleData, FundingRateData};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

fn create_test_candle_data(symbol: &str, interval: &str, timestamp: i64) -> CandleData {
    CandleData {
        symbol: symbol.to_string(),
        timestamp,
        open: 45000.0,
        high: 45100.0,
        low: 44900.0,
        close: 45050.0,
        volume: 150.5,
        interval: interval.to_string(),
        exchange: "binance".to_string(),
    }
}

fn create_test_funding_rate_data(symbol: &str, timestamp: i64) -> FundingRateData {
    FundingRateData {
        symbol: symbol.to_string(),
        timestamp,
        rate: 0.0001,                            // 0.01%
        next_funding_time: timestamp + 28800000, // 8 hours later
        exchange: "binance".to_string(),
    }
}

#[tokio::test]
async fn test_handler_creation() {
    let handler = LowFrequencyHandler::new();
    let metrics = handler.get_metrics();

    assert_eq!(metrics.get("candles_processed").unwrap(), &0);
    assert_eq!(metrics.get("funding_rates_processed").unwrap(), &0);
    assert_eq!(metrics.get("total_failed").unwrap(), &0);
}

#[tokio::test]
async fn test_candle_ingestion() {
    let handler = LowFrequencyHandler::new();
    handler.start().await.unwrap();

    let candle = create_test_candle_data("BTCUSDT", "1m", 1640995200000);
    let result = handler.ingest_candle(&candle).await;
    assert!(result.is_ok());

    // Give processing time
    sleep(Duration::from_millis(200)).await;

    let stored_candles = handler.get_candles("BTCUSDT", "1m").await;
    assert!(stored_candles.is_some());
    assert_eq!(stored_candles.unwrap().len(), 1);

    let metrics = handler.get_metrics();
    assert_eq!(metrics.get("candles_processed").unwrap(), &1);

    handler.stop().await.unwrap();
}

#[tokio::test]
async fn test_funding_rate_ingestion() {
    let handler = LowFrequencyHandler::new();
    handler.start().await.unwrap();

    let funding = create_test_funding_rate_data("BTCUSDT", 1640995200000);
    let result = handler.ingest_funding_rate(&funding).await;
    assert!(result.is_ok());

    // Give processing time
    sleep(Duration::from_millis(200)).await;

    let stored_funding = handler.get_funding_rates("BTCUSDT").await;
    assert!(stored_funding.is_some());
    assert_eq!(stored_funding.unwrap().len(), 1);

    let metrics = handler.get_metrics();
    assert_eq!(metrics.get("funding_rates_processed").unwrap(), &1);

    handler.stop().await.unwrap();
}

#[tokio::test]
async fn test_invalid_candle_data() {
    let handler = LowFrequencyHandler::new();

    // Test empty symbol
    let mut candle = create_test_candle_data("", "1m", 1640995200000);
    let result = handler.ingest_candle(&candle).await;
    assert!(result.is_err());

    // Test empty interval
    candle = create_test_candle_data("BTCUSDT", "", 1640995200000);
    let result = handler.ingest_candle(&candle).await;
    assert!(result.is_err());

    // Test invalid timestamp
    candle = create_test_candle_data("BTCUSDT", "1m", -1);
    let result = handler.ingest_candle(&candle).await;
    assert!(result.is_err());

    // Test invalid prices
    candle = create_test_candle_data("BTCUSDT", "1m", 1640995200000);
    candle.open = -100.0;
    let result = handler.ingest_candle(&candle).await;
    assert!(result.is_err());

    // Test invalid high/low relationship
    candle = create_test_candle_data("BTCUSDT", "1m", 1640995200000);
    candle.high = 44000.0;
    candle.low = 45000.0;
    let result = handler.ingest_candle(&candle).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_funding_rate_data() {
    let handler = LowFrequencyHandler::new();

    // Test empty symbol
    let mut funding = create_test_funding_rate_data("", 1640995200000);
    let result = handler.ingest_funding_rate(&funding).await;
    assert!(result.is_err());

    // Test invalid timestamp
    funding = create_test_funding_rate_data("BTCUSDT", -1);
    let result = handler.ingest_funding_rate(&funding).await;
    assert!(result.is_err());

    // Test invalid next funding time
    funding = create_test_funding_rate_data("BTCUSDT", 1640995200000);
    funding.next_funding_time = 1640995100000; // Before current timestamp
    let result = handler.ingest_funding_rate(&funding).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_storage_size_limits() {
    let config = LowFrequencyConfig {
        max_items_per_symbol: 3,
        ..Default::default()
    };
    let handler = LowFrequencyHandler::with_config(config);
    handler.start().await.unwrap();

    // Add more candles than the limit
    for i in 0..5 {
        let candle = create_test_candle_data("BTCUSDT", "1m", 1640995200000 + i * 60000);
        handler.ingest_candle(&candle).await.unwrap();
    }

    // Give processing time
    sleep(Duration::from_millis(200)).await;

    let stored_candles = handler.get_candles("BTCUSDT", "1m").await;
    assert!(stored_candles.is_some());
    assert_eq!(stored_candles.unwrap().len(), 3); // Should be limited to 3

    handler.stop().await.unwrap();
}

#[tokio::test]
async fn test_multiple_symbols_and_intervals() {
    let handler = LowFrequencyHandler::new();
    handler.start().await.unwrap();

    // Add candles for different symbols and intervals
    let candle1 = create_test_candle_data("BTCUSDT", "1m", 1640995200000);
    let candle2 = create_test_candle_data("BTCUSDT", "5m", 1640995200000);
    let candle3 = create_test_candle_data("ETHUSDT", "1m", 1640995200000);

    handler.ingest_candle(&candle1).await.unwrap();
    handler.ingest_candle(&candle2).await.unwrap();
    handler.ingest_candle(&candle3).await.unwrap();

    // Add funding rates for different symbols
    let funding1 = create_test_funding_rate_data("BTCUSDT", 1640995200000);
    let funding2 = create_test_funding_rate_data("ETHUSDT", 1640995200000);

    handler.ingest_funding_rate(&funding1).await.unwrap();
    handler.ingest_funding_rate(&funding2).await.unwrap();

    // Give processing time
    sleep(Duration::from_millis(200)).await;

    // Check that data is stored correctly
    assert!(handler.get_candles("BTCUSDT", "1m").await.is_some());
    assert!(handler.get_candles("BTCUSDT", "5m").await.is_some());
    assert!(handler.get_candles("ETHUSDT", "1m").await.is_some());
    assert!(handler.get_candles("ETHUSDT", "5m").await.is_none());

    assert!(handler.get_funding_rates("BTCUSDT").await.is_some());
    assert!(handler.get_funding_rates("ETHUSDT").await.is_some());

    let active_symbols = handler.get_active_symbols().await;
    assert!(active_symbols.contains(&"BTCUSDT".to_string()));
    assert!(active_symbols.contains(&"ETHUSDT".to_string()));

    handler.stop().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_ingestion() {
    let handler = Arc::new(LowFrequencyHandler::new());
    handler.start().await.unwrap();

    let mut handles = vec![];

    // Spawn multiple tasks for concurrent ingestion
    for i in 0..10 {
        let handler_clone = Arc::clone(&handler);
        let handle = tokio::spawn(async move {
            let candle = create_test_candle_data("BTCUSDT", "1m", 1640995200000 + i * 60000);
            handler_clone.ingest_candle(&candle).await
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Give processing time
    sleep(Duration::from_millis(300)).await;

    let stored_candles = handler.get_candles("BTCUSDT", "1m").await;
    assert!(stored_candles.is_some());
    assert_eq!(stored_candles.unwrap().len(), 10);

    let metrics = handler.get_metrics();
    assert_eq!(metrics.get("candles_processed").unwrap(), &10);

    handler.stop().await.unwrap();
}

#[tokio::test]
async fn test_storage_stats() {
    let handler = LowFrequencyHandler::new();
    handler.start().await.unwrap();

    let candle = create_test_candle_data("BTCUSDT", "1m", 1640995200000);
    let funding = create_test_funding_rate_data("ETHUSDT", 1640995200000);

    handler.ingest_candle(&candle).await.unwrap();
    handler.ingest_funding_rate(&funding).await.unwrap();

    // Give processing time
    sleep(Duration::from_millis(200)).await;

    let stats = handler.get_storage_stats().await;
    assert_eq!(stats.get("candle_symbols").unwrap(), &1);
    assert_eq!(stats.get("funding_rate_symbols").unwrap(), &1);
    assert_eq!(stats.get("total_candle_entries").unwrap(), &1);
    assert_eq!(stats.get("total_funding_rate_entries").unwrap(), &1);

    handler.stop().await.unwrap();
}

#[test]
fn test_storage_operations() {
    let storage = LowFrequencyStorage::new(5);

    // Test candle storage
    let candle = create_test_candle_data("BTCUSDT", "1m", 1640995200000);
    storage.add_candle(&candle);

    let stored_candles = storage.get_candles("BTCUSDT", "1m");
    assert!(stored_candles.is_some());
    assert_eq!(stored_candles.unwrap().len(), 1);

    // Test funding rate storage
    let funding = create_test_funding_rate_data("BTCUSDT", 1640995200000);
    storage.add_funding_rate(&funding);

    let stored_funding = storage.get_funding_rates("BTCUSDT");
    assert!(stored_funding.is_some());
    assert_eq!(stored_funding.unwrap().len(), 1);

    // Test symbol listing
    let candle_symbols = storage.get_candle_symbols();
    let funding_symbols = storage.get_funding_rate_symbols();
    let all_symbols = storage.get_all_symbols();

    assert!(candle_symbols.contains(&"BTCUSDT".to_string()));
    assert!(funding_symbols.contains(&"BTCUSDT".to_string()));
    assert!(all_symbols.contains(&"BTCUSDT".to_string()));
}

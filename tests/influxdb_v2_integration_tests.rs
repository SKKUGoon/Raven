// Integration tests for InfluxDB v2 client
// These tests require a running InfluxDB v2 instance

use market_data_subscription_server::database::influx_client::{
    create_orderbook_datapoint, create_trade_datapoint, InfluxClient, InfluxConfig,
};
use market_data_subscription_server::types::{
    CandleData, FundingRateData, OrderBookSnapshot, TradeSnapshot,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Helper function to create test config
fn create_test_config() -> InfluxConfig {
    InfluxConfig {
        url: std::env::var("INFLUXDB_URL").unwrap_or_else(|_| "http://localhost:8086".to_string()),
        bucket: std::env::var("INFLUXDB_BUCKET").unwrap_or_else(|_| "test_market_data".to_string()),
        org: std::env::var("INFLUXDB_ORG").unwrap_or_else(|_| "test_org".to_string()),
        token: std::env::var("INFLUXDB_TOKEN").ok(),
        pool_size: 2,
        timeout: Duration::from_secs(10),
        retry_attempts: 2,
        retry_delay: Duration::from_millis(100),
        batch_size: 100,
        flush_interval: Duration::from_millis(10),
    }
}

// Helper function to check if InfluxDB is available
async fn is_influxdb_available() -> bool {
    let config = create_test_config();
    let client = InfluxClient::new(config);

    match client.connect().await {
        Ok(_) => {
            // Try a ping to make sure it's really working
            client.ping().await.is_ok()
        }
        Err(_) => false,
    }
}

#[tokio::test]
#[ignore] // Ignored by default, run with --ignored flag when InfluxDB is available
async fn test_influxdb_connection() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);

    // Test connection
    let result = client.connect().await;
    assert!(
        result.is_ok(),
        "Failed to connect to InfluxDB: {:?}",
        result
    );

    // Test ping
    let ping_result = client.ping().await;
    assert!(
        ping_result.is_ok(),
        "Failed to ping InfluxDB: {:?}",
        ping_result
    );

    // Test health check
    let health_result = client.health_check().await;
    assert!(
        health_result.is_ok(),
        "Health check failed: {:?}",
        health_result
    );
}

#[tokio::test]
#[ignore]
async fn test_write_orderbook_snapshot() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);
    client.connect().await.unwrap();

    let snapshot = OrderBookSnapshot {
        symbol: "BTCUSDT_TEST".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        best_bid_price: 45000.0,
        best_bid_quantity: 1.5,
        best_ask_price: 45001.0,
        best_ask_quantity: 1.2,
        sequence: 12345,
    };

    let result = client.write_orderbook_snapshot(&snapshot).await;
    assert!(
        result.is_ok(),
        "Failed to write orderbook snapshot: {:?}",
        result
    );
}

#[tokio::test]
#[ignore]
async fn test_write_trade_snapshot() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);
    client.connect().await.unwrap();

    let trade = TradeSnapshot {
        symbol: "BTCUSDT_TEST".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        price: 45000.5,
        quantity: 0.1,
        side: "buy".to_string(),
        trade_id: 123456,
    };

    let result = client.write_trade_snapshot(&trade).await;
    assert!(
        result.is_ok(),
        "Failed to write trade snapshot: {:?}",
        result
    );
}

#[tokio::test]
#[ignore]
async fn test_write_candle_data() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);
    client.connect().await.unwrap();

    let candle = CandleData {
        symbol: "BTCUSDT_TEST".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        open: 45000.0,
        high: 45100.0,
        low: 44900.0,
        close: 45050.0,
        volume: 150.5,
        interval: "1m".to_string(),
        exchange: "test_exchange".to_string(),
    };

    let result = client.write_candle(&candle).await;
    assert!(result.is_ok(), "Failed to write candle data: {:?}", result);
}

#[tokio::test]
#[ignore]
async fn test_write_funding_rate() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);
    client.connect().await.unwrap();

    let funding = FundingRateData {
        symbol: "BTCUSDT_TEST".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        rate: 0.0001,
        next_funding_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            + 28800000, // 8 hours later
        exchange: "test_exchange".to_string(),
    };

    let result = client.write_funding_rate(&funding).await;
    assert!(result.is_ok(), "Failed to write funding rate: {:?}", result);
}

#[tokio::test]
#[ignore]
async fn test_write_wallet_update() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);
    client.connect().await.unwrap();

    let balances = vec![
        ("BTC".to_string(), 1.5, 0.1),
        ("USDT".to_string(), 10000.0, 500.0),
    ];

    let result = client
        .write_wallet_update(
            "test_user_123",
            &balances,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        )
        .await;
    assert!(
        result.is_ok(),
        "Failed to write wallet update: {:?}",
        result
    );
}

#[tokio::test]
#[ignore]
async fn test_batch_write() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);
    client.connect().await.unwrap();

    // Create multiple datapoints for batch write
    let mut datapoints = Vec::new();

    // Add orderbook datapoints
    for i in 0..5 {
        let snapshot = OrderBookSnapshot {
            symbol: format!("BATCH_TEST_{}", i),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
                + i as i64,
            best_bid_price: 45000.0 + i as f64,
            best_bid_quantity: 1.5,
            best_ask_price: 45001.0 + i as f64,
            best_ask_quantity: 1.2,
            sequence: 12345 + i as u64,
        };
        datapoints.push(create_orderbook_datapoint(&snapshot).unwrap());
    }

    // Add trade datapoints
    for i in 0..5 {
        let trade = TradeSnapshot {
            symbol: format!("BATCH_TEST_{}", i),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
                + i as i64,
            price: 45000.5 + i as f64,
            quantity: 0.1,
            side: if i % 2 == 0 { "buy" } else { "sell" }.to_string(),
            trade_id: 123456 + i as u64,
        };
        datapoints.push(create_trade_datapoint(&trade).unwrap());
    }

    let result = client.write_batch(datapoints).await;
    assert!(result.is_ok(), "Failed to write batch: {:?}", result);
}

#[tokio::test]
#[ignore]
async fn test_database_setup() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);
    client.connect().await.unwrap();

    let result = client.setup_database().await;
    assert!(result.is_ok(), "Failed to setup database: {:?}", result);
}

#[tokio::test]
#[ignore]
async fn test_query_historical_data() {
    if !is_influxdb_available().await {
        println!("InfluxDB not available, skipping integration test");
        return;
    }

    let config = create_test_config();
    let client = InfluxClient::new(config);
    client.connect().await.unwrap();

    // First write some test data
    let snapshot = OrderBookSnapshot {
        symbol: "QUERY_TEST".to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        best_bid_price: 45000.0,
        best_bid_quantity: 1.5,
        best_ask_price: 45001.0,
        best_ask_quantity: 1.2,
        sequence: 12345,
    };

    let write_result = client.write_orderbook_snapshot(&snapshot).await;
    assert!(write_result.is_ok());

    // Wait a bit for data to be written
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now try to query it back
    let start_time = snapshot.timestamp - 60000; // 1 minute before
    let end_time = snapshot.timestamp + 60000; // 1 minute after

    let result = client
        .query_historical_data("orderbook", "QUERY_TEST", start_time, end_time, Some(10))
        .await;

    // Note: This will currently return empty results due to placeholder implementation
    // Once proper FromMap is implemented, this should return the data we wrote
    assert!(
        result.is_ok(),
        "Failed to query historical data: {:?}",
        result
    );

    // For now, just verify it doesn't crash
    let _results = result.unwrap();
    // TODO: Add proper assertions once query implementation is complete
}

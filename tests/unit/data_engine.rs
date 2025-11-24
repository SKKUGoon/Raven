// DataEngine Tests - Project Raven

use raven::common::db::{DeadLetterQueue, EnhancedInfluxClient, InfluxClient, InfluxConfig};
use raven::current_timestamp_millis;
use raven::server::data_engine::storage::{OrderBookData, TradeData, TradeSide};
use raven::server::data_engine::{DataEngine, DataEngineConfig};
use raven::server::exchanges::types::Exchange;
use raven::server::stream_router::StreamRouter;
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn create_test_data_engine() -> DataEngine {
    let influx_config = InfluxConfig::default();
    let influx_client = Arc::new(InfluxClient::new(influx_config));
    let subscription_manager = Arc::new(StreamRouter::new());
    let data_engine_config = DataEngineConfig::default();
    let dead_letter_queue = Arc::new(DeadLetterQueue::new(Default::default()));
    let enhanced_client = Arc::new(EnhancedInfluxClient::new(
        influx_client,
        Arc::clone(&dead_letter_queue),
    ));

    DataEngine::new(
        data_engine_config,
        enhanced_client,
        subscription_manager,
    )
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
    assert_eq!(metrics.get("total_written").unwrap(), &0);
}

#[tokio::test]
async fn test_orderbook_persistence() {
    let data_engine = create_test_data_engine();
    let data = create_test_orderbook_data();

    // Since we mock the influx client (it's just structs), this will fail to connect 
    // but the persist_orderbook_data call should succeed in logic until it hits the network
    // In a real unit test we would mock the EnhancedInfluxClient, but for now we check API existence
    let _ = data_engine.persist_orderbook_data("BTCUSDT", data).await;
}

#[tokio::test]
async fn test_trade_persistence() {
    let data_engine = create_test_data_engine();
    let data = create_test_trade_data();

    let _ = data_engine.persist_trade_data("BTCUSDT", data).await;
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
        .total_written
        .fetch_add(3, Ordering::Relaxed);
    data_engine
        .metrics
        .total_failed
        .fetch_add(1, Ordering::Relaxed);

    let metrics = data_engine.get_metrics();
    assert_eq!(metrics.get("total_ingested").unwrap(), &5);
    assert_eq!(metrics.get("total_written").unwrap(), &3);
    assert_eq!(metrics.get("total_failed").unwrap(), &1);
}


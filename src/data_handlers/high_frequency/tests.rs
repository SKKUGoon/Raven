use super::*;
use crate::citadel::storage::{HighFrequencyStorage, OrderBookData, TradeData, TradeSide};
use crate::exchanges::types::Exchange;
use std::sync::Arc;
use std::thread;

fn create_test_orderbook_data(symbol: &str, sequence: u64) -> OrderBookData {
    OrderBookData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000 + sequence as i64,
        // Bids in descending order (highest first)
        bids: vec![
            (45000.0 + sequence as f64, 1.5),
            (44999.0 + sequence as f64, 2.0),
        ],
        // Asks in ascending order (lowest first)
        asks: vec![
            (45001.0 + sequence as f64, 1.2),
            (45002.0 + sequence as f64, 1.8),
        ],
        sequence,
        exchange: Exchange::BinanceSpot,
    }
}

fn create_test_trade_data(symbol: &str, price: f64) -> TradeData {
    TradeData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000,
        price,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: format!("trade_{price}"),
        exchange: Exchange::BinanceSpot,
    }
}

#[test]
fn test_handler_creation() {
    let handler = HighFrequencyHandler::new();
    let stats = handler.get_performance_stats();
    assert_eq!(stats.orderbook_symbols, 0);
    assert_eq!(stats.trade_symbols, 0);
}

#[test]
fn test_orderbook_ingestion() {
    let handler = HighFrequencyHandler::new();
    let data = create_test_orderbook_data("BTCUSDT", 12345);

    let result = handler.ingest_orderbook_atomic("BTCUSDT", &data);
    assert!(result.is_ok());

    let snapshot = handler.capture_orderbook_snapshot("BTCUSDT", &Exchange::BinanceSpot);
    assert!(snapshot.is_ok());

    let snapshot = snapshot.unwrap();
    assert_eq!(snapshot.symbol, "BTCUSDT");
    assert_eq!(snapshot.sequence, 12345);
    assert_eq!(snapshot.best_bid_price, 45000.0 + 12345.0);
}

#[test]
fn test_trade_ingestion() {
    let handler = HighFrequencyHandler::new();
    let data = create_test_trade_data("BTCUSDT", 45000.5);

    let result = handler.ingest_trade_atomic("BTCUSDT", &data);
    assert!(result.is_ok());

    let snapshot = handler.capture_trade_snapshot("BTCUSDT", &Exchange::BinanceSpot);
    assert!(snapshot.is_ok());

    let snapshot = snapshot.unwrap();
    assert_eq!(snapshot.symbol, "BTCUSDT");
    assert_eq!(snapshot.price, 45000.5);
    assert_eq!(snapshot.side, "buy");
}

#[test]
fn test_invalid_orderbook_data() {
    let handler = HighFrequencyHandler::new();

    // Test empty symbol
    let data = create_test_orderbook_data("", 12345);
    let result = handler.ingest_orderbook_atomic("", &data);
    assert!(result.is_err());

    // Test empty orderbook
    let mut data = create_test_orderbook_data("BTCUSDT", 12345);
    data.bids.clear();
    data.asks.clear();
    let result = handler.ingest_orderbook_atomic("BTCUSDT", &data);
    assert!(result.is_err());
}

#[test]
fn test_invalid_trade_data() {
    let handler = HighFrequencyHandler::new();

    // Test empty symbol
    let data = create_test_trade_data("", 45000.0);
    let result = handler.ingest_trade_atomic("", &data);
    assert!(result.is_err());

    // Test invalid price
    let mut data = create_test_trade_data("BTCUSDT", -45000.0);
    let result = handler.ingest_trade_atomic("BTCUSDT", &data);
    assert!(result.is_err());

    // Test invalid quantity
    data = create_test_trade_data("BTCUSDT", 45000.0);
    data.quantity = -0.1;
    let result = handler.ingest_trade_atomic("BTCUSDT", &data);
    assert!(result.is_err());

    // Test invalid price instead of invalid side
    data = create_test_trade_data("BTCUSDT", 45000.0);
    data.price = -1.0;
    let result = handler.ingest_trade_atomic("BTCUSDT", &data);
    assert!(result.is_err());

    // Test empty trade ID
    data = create_test_trade_data("BTCUSDT", 45000.0);
    data.trade_id = "".to_string();
    let result = handler.ingest_trade_atomic("BTCUSDT", &data);
    assert!(result.is_err());
}

#[test]
fn test_price_level_validation() {
    let handler = HighFrequencyHandler::new();

    // Test valid price levels
    let bids = vec![(45000.0, 1.0), (44999.0, 2.0), (44998.0, 1.5)];
    let asks = vec![(45001.0, 1.0), (45002.0, 2.0), (45003.0, 1.5)];
    let result = handler.validate_price_levels(&bids, &asks);
    assert!(result.is_ok() && result.unwrap());

    // Test invalid bid ordering (ascending instead of descending)
    let invalid_bids = vec![(44998.0, 1.0), (44999.0, 2.0), (45000.0, 1.5)];
    let result = handler.validate_price_levels(&invalid_bids, &asks);
    assert!(result.is_ok() && !result.unwrap());

    // Test invalid ask ordering (descending instead of ascending)
    let invalid_asks = vec![(45003.0, 1.0), (45002.0, 2.0), (45001.0, 1.5)];
    let result = handler.validate_price_levels(&bids, &invalid_asks);
    assert!(result.is_ok() && !result.unwrap());

    // Test invalid spread (ask < bid)
    let invalid_spread_bids = vec![(45002.0, 1.0)];
    let invalid_spread_asks = vec![(45001.0, 1.0)];
    let result = handler.validate_price_levels(&invalid_spread_bids, &invalid_spread_asks);
    assert!(result.is_ok() && !result.unwrap());
}

#[test]
fn test_concurrent_operations() {
    let handler = Arc::new(HighFrequencyHandler::new());
    let mut handles = vec![];

    // Spawn multiple threads for concurrent orderbook updates
    for i in 0..10 {
        let handler_clone = Arc::clone(&handler);
        let handle = thread::spawn(move || {
            let data = create_test_orderbook_data("BTCUSDT", 12345 + i);
            handler_clone.ingest_orderbook_atomic("BTCUSDT", &data)
        });
        handles.push(handle);
    }

    // Spawn multiple threads for concurrent trade updates
    for i in 0..10 {
        let handler_clone = Arc::clone(&handler);
        let handle = thread::spawn(move || {
            let data = create_test_trade_data("BTCUSDT", 45000.0 + i as f64);
            handler_clone.ingest_trade_atomic("BTCUSDT", &data)
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }

    // Verify data was updated
    let orderbook_snapshot = handler.capture_orderbook_snapshot("BTCUSDT", &Exchange::BinanceSpot);
    assert!(orderbook_snapshot.is_ok());

    let trade_snapshot = handler.capture_trade_snapshot("BTCUSDT", &Exchange::BinanceSpot);
    assert!(trade_snapshot.is_ok());
}

#[test]
fn test_shared_storage() {
    let storage = Arc::new(HighFrequencyStorage::new());
    let handler1 = HighFrequencyHandler::with_storage(Arc::clone(&storage));
    let handler2 = HighFrequencyHandler::with_storage(Arc::clone(&storage));

    // Update through handler1
    let data = create_test_orderbook_data("BTCUSDT", 12345);
    let result = handler1.ingest_orderbook_atomic("BTCUSDT", &data);
    assert!(result.is_ok());

    // Read through handler2
    let snapshot = handler2.capture_orderbook_snapshot("BTCUSDT", &Exchange::BinanceSpot);
    assert!(snapshot.is_ok());
    assert_eq!(snapshot.unwrap().sequence, 12345);
}

#[test]
fn test_symbol_listing() {
    let handler = HighFrequencyHandler::new();

    // Initially no symbols
    assert_eq!(handler.get_orderbook_symbols().len(), 0);
    assert_eq!(handler.get_trade_symbols().len(), 0);

    // Add orderbook data
    let orderbook_data = create_test_orderbook_data("BTCUSDT", 12345);
    handler
        .ingest_orderbook_atomic("BTCUSDT", &orderbook_data)
        .unwrap();

    // Add trade data for different symbol
    let trade_data = create_test_trade_data("ETHUSDT", 3000.0);
    handler.ingest_trade_atomic("ETHUSDT", &trade_data).unwrap();

    let orderbook_symbols = handler.get_orderbook_symbols();
    let trade_symbols = handler.get_trade_symbols();

    let expected_orderbook = format!("{}:{}", Exchange::BinanceSpot, "BTCUSDT");
    let expected_trade = format!("{}:{}", Exchange::BinanceSpot, "ETHUSDT");

    assert_eq!(orderbook_symbols.len(), 1);
    assert!(orderbook_symbols.contains(&expected_orderbook));

    assert_eq!(trade_symbols.len(), 1);
    assert!(trade_symbols.contains(&expected_trade));
}

#[test]
fn test_performance_stats() {
    let handler = HighFrequencyHandler::new();

    // Initially no data
    let stats = handler.get_performance_stats();
    assert_eq!(stats.orderbook_symbols, 0);
    assert_eq!(stats.trade_symbols, 0);

    // Add some data
    let orderbook_data = create_test_orderbook_data("BTCUSDT", 12345);
    handler
        .ingest_orderbook_atomic("BTCUSDT", &orderbook_data)
        .unwrap();

    let trade_data = create_test_trade_data("ETHUSDT", 3000.0);
    handler.ingest_trade_atomic("ETHUSDT", &trade_data).unwrap();

    let stats = handler.get_performance_stats();
    assert_eq!(stats.orderbook_symbols, 1);
    assert_eq!(stats.trade_symbols, 1);
}

#[test]
fn test_snapshot_not_found() {
    let handler = HighFrequencyHandler::new();

    // Try to capture snapshot for non-existent symbol
    let result = handler.capture_orderbook_snapshot("NONEXISTENT", &Exchange::BinanceSpot);
    assert!(result.is_err());

    let result = handler.capture_trade_snapshot("NONEXISTENT", &Exchange::BinanceSpot);
    assert!(result.is_err());
}

#[test]
fn test_multi_exchange_support() {
    let handler = HighFrequencyHandler::new();

    // Create data for different exchanges
    let mut binance_data = create_test_orderbook_data("BTCUSDT", 12345);
    binance_data.exchange = Exchange::BinanceSpot;

    let mut coinbase_data = create_test_orderbook_data("BTCUSDT", 12346);
    coinbase_data.exchange = Exchange::BinanceFutures;

    let mut kraken_data = create_test_trade_data("BTCUSDT", 45000.0);
    kraken_data.exchange = Exchange::BinanceSpot;

    // Ingest data from different exchanges
    handler
        .ingest_orderbook_atomic("BTCUSDT", &binance_data)
        .unwrap();
    handler
        .ingest_orderbook_atomic("BTCUSDT", &coinbase_data)
        .unwrap();
    handler
        .ingest_trade_atomic("BTCUSDT", &kraken_data)
        .unwrap();

    // Check active exchanges
    let exchanges = handler.get_active_exchanges();
    assert!(exchanges.contains(&Exchange::BinanceSpot.to_string()));
    assert!(exchanges.contains(&Exchange::BinanceFutures.to_string()));
    assert!(exchanges.contains(&Exchange::BinanceSpot.to_string()));

    // Check symbols for specific exchange
    let binance_symbols = handler.get_symbols_for_exchange(&Exchange::BinanceSpot.to_string());
    assert!(binance_symbols.contains(&"BTCUSDT".to_string()));

    let coinbase_symbols = handler.get_symbols_for_exchange(&Exchange::BinanceFutures.to_string());
    assert!(coinbase_symbols.contains(&"BTCUSDT".to_string()));

    // Test exchange-specific snapshots
    let binance_snapshot = handler
        .capture_orderbook_snapshot_for_exchange(&Exchange::BinanceSpot.to_string(), "BTCUSDT");
    assert!(binance_snapshot.is_ok());
    assert_eq!(binance_snapshot.unwrap().sequence, 12345);

    let coinbase_snapshot = handler
        .capture_orderbook_snapshot_for_exchange(&Exchange::BinanceFutures.to_string(), "BTCUSDT");
    assert!(coinbase_snapshot.is_ok());
    assert_eq!(coinbase_snapshot.unwrap().sequence, 12346);

    let kraken_trade_snapshot =
        handler.capture_trade_snapshot_for_exchange(&Exchange::BinanceSpot.to_string(), "BTCUSDT");
    assert!(kraken_trade_snapshot.is_ok());
}

#[test]
fn test_exchange_isolation() {
    let handler = HighFrequencyHandler::new();

    // Create data for same symbol on different exchanges with different prices
    let mut binance_data = create_test_orderbook_data("BTCUSDT", 12345);
    binance_data.exchange = Exchange::BinanceSpot;
    binance_data.bids = vec![(50000.0, 1.0)];
    binance_data.asks = vec![(50001.0, 1.0)];

    let mut coinbase_data = create_test_orderbook_data("BTCUSDT", 12346);
    coinbase_data.exchange = Exchange::BinanceFutures;
    coinbase_data.bids = vec![(49000.0, 1.0)];
    coinbase_data.asks = vec![(49001.0, 1.0)];

    // Ingest data
    handler
        .ingest_orderbook_atomic("BTCUSDT", &binance_data)
        .unwrap();
    handler
        .ingest_orderbook_atomic("BTCUSDT", &coinbase_data)
        .unwrap();

    // Verify data isolation - each exchange should have different prices
    let binance_snapshot = handler
        .capture_orderbook_snapshot_for_exchange(&Exchange::BinanceSpot.to_string(), "BTCUSDT")
        .unwrap();
    let coinbase_snapshot = handler
        .capture_orderbook_snapshot_for_exchange(&Exchange::BinanceFutures.to_string(), "BTCUSDT")
        .unwrap();

    assert_eq!(binance_snapshot.best_bid_price, 50000.0);
    assert_eq!(binance_snapshot.best_ask_price, 50001.0);
    assert_eq!(binance_snapshot.sequence, 12345);

    assert_eq!(coinbase_snapshot.best_bid_price, 49000.0);
    assert_eq!(coinbase_snapshot.best_ask_price, 49001.0);
    assert_eq!(coinbase_snapshot.sequence, 12346);
}

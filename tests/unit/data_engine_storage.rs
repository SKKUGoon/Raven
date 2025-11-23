use raven::server::data_engine::storage::{
    atomic_to_price, atomic_to_quantity, price_to_atomic, quantity_to_atomic, AtomicOrderBook,
    AtomicTrade, HighFrequencyStorage, OrderBookData, OrderBookSnapshot, TradeData, TradeSide,
    TradeSnapshot, PRICE_SCALE, QUANTITY_SCALE,
};
use raven::server::exchanges::types::Exchange;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;

#[test]
fn test_atomic_orderbook_creation() {
    let orderbook = AtomicOrderBook::new("BTCUSDT".to_string(), Exchange::BinanceSpot);
    assert_eq!(orderbook.symbol, "BTCUSDT");
    assert_eq!(orderbook.timestamp.load(Ordering::Relaxed), 0);
    assert_eq!(orderbook.sequence.load(Ordering::Relaxed), 0);
}

#[test]
fn test_atomic_orderbook_update() {
    let orderbook = AtomicOrderBook::new("BTCUSDT".to_string(), Exchange::BinanceSpot);
    let data = OrderBookData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000, // 2022-01-01 00:00:00 UTC
        bids: vec![(45000.0, 1.5), (44999.0, 2.0)],
        asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
        sequence: 12345,
        exchange: Exchange::BinanceSpot,
    };

    orderbook.update_from_data(&data);

    assert_eq!(orderbook.timestamp.load(Ordering::Relaxed), 1640995200000);
    assert_eq!(orderbook.sequence.load(Ordering::Relaxed), 12345);
    assert_eq!(
        orderbook.best_bid_price.load(Ordering::Relaxed),
        (45000.0 * PRICE_SCALE) as u64
    );
    assert_eq!(
        orderbook.best_ask_price.load(Ordering::Relaxed),
        (45001.0 * PRICE_SCALE) as u64
    );
}

#[test]
fn test_atomic_orderbook_snapshot() {
    let orderbook = AtomicOrderBook::new("BTCUSDT".to_string(), Exchange::BinanceSpot);
    let data = OrderBookData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        bids: vec![(45000.0, 1.5)],
        asks: vec![(45001.0, 1.2)],
        sequence: 12345,
        exchange: Exchange::BinanceSpot,
    };

    orderbook.update_from_data(&data);
    let snapshot = orderbook.to_snapshot();

    assert_eq!(snapshot.symbol, "BTCUSDT");
    assert_eq!(snapshot.timestamp, 1640995200000);
    assert_eq!(snapshot.best_bid_price, 45000.0);
    assert_eq!(snapshot.best_ask_price, 45001.0);
    assert_eq!(snapshot.sequence, 12345);
}

#[test]
fn test_atomic_orderbook_spread() {
    let orderbook = AtomicOrderBook::new("BTCUSDT".to_string(), Exchange::BinanceSpot);
    let data = OrderBookData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        bids: vec![(45000.0, 1.5)],
        asks: vec![(45001.0, 1.2)],
        sequence: 12345,
        exchange: Exchange::BinanceSpot,
    };

    orderbook.update_from_data(&data);
    let spread = orderbook.get_spread();
    assert!((spread - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_atomic_trade_creation() {
    let trade = AtomicTrade::new("BTCUSDT".to_string(), Exchange::BinanceSpot);
    assert_eq!(trade.symbol, "BTCUSDT");
    assert_eq!(trade.timestamp.load(Ordering::Relaxed), 0);
    assert_eq!(trade.side.load(Ordering::Relaxed), 0);
}

#[test]
fn test_atomic_trade_update() {
    let trade = AtomicTrade::new("BTCUSDT".to_string(), Exchange::BinanceSpot);
    let data = TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: "abc123".to_string(),
        exchange: Exchange::BinanceSpot,
    };

    trade.update_from_data(&data);

    assert_eq!(trade.timestamp.load(Ordering::Relaxed), 1640995200000);
    assert_eq!(
        trade.price.load(Ordering::Relaxed),
        (45000.5 * PRICE_SCALE) as u64
    );
    assert_eq!(
        trade.quantity.load(Ordering::Relaxed),
        (0.1 * QUANTITY_SCALE) as u64
    );
    assert_eq!(trade.side.load(Ordering::Relaxed), 0); // buy = 0
}

#[test]
fn test_atomic_trade_side_conversion() {
    let trade = AtomicTrade::new("BTCUSDT".to_string(), Exchange::BinanceSpot);

    // Test buy side
    let buy_data = TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.0,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: "buy123".to_string(),
        exchange: Exchange::BinanceSpot,
    };
    trade.update_from_data(&buy_data);
    assert_eq!(trade.side.load(Ordering::Relaxed), 0);

    // Test sell side
    let sell_data = TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.0,
        quantity: 0.1,
        side: TradeSide::Sell,
        trade_id: "sell123".to_string(),
        exchange: Exchange::BinanceSpot,
    };
    trade.update_from_data(&sell_data);
    assert_eq!(trade.side.load(Ordering::Relaxed), 1);
}

#[test]
fn test_atomic_trade_snapshot() {
    let trade = AtomicTrade::new("BTCUSDT".to_string(), Exchange::BinanceSpot);
    let data = TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Sell,
        trade_id: "abc123".to_string(),
        exchange: Exchange::BinanceSpot,
    };

    trade.update_from_data(&data);
    let snapshot = trade.to_snapshot();

    assert_eq!(snapshot.symbol, "BTCUSDT");
    assert_eq!(snapshot.timestamp, 1640995200000);
    assert_eq!(snapshot.price, 45000.5);
    assert_eq!(snapshot.quantity, 0.1);
    assert_eq!(snapshot.side, "sell");
}

#[test]
fn test_atomic_trade_value() {
    let trade = AtomicTrade::new("BTCUSDT".to_string(), Exchange::BinanceSpot);
    let data = TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.0,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: "abc123".to_string(),
        exchange: Exchange::BinanceSpot,
    };

    trade.update_from_data(&data);
    let trade_value = trade.get_trade_value();
    assert!((trade_value - 4500.0).abs() < 0.01); // 45000 * 0.1 = 4500
}

#[test]
fn test_high_frequency_storage() {
    let storage = HighFrequencyStorage::new();

    // Test orderbook operations
    let orderbook_data = OrderBookData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        bids: vec![(45000.0, 1.5)],
        asks: vec![(45001.0, 1.2)],
        sequence: 12345,
        exchange: Exchange::BinanceSpot,
    };

    storage.update_orderbook(&orderbook_data);
    let snapshot = storage
        .get_orderbook_snapshot("BTCUSDT", &Exchange::BinanceSpot)
        .unwrap();
    assert_eq!(snapshot.symbol, "BTCUSDT");
    assert_eq!(snapshot.best_bid_price, 45000.0);

    // Test trade operations
    let trade_data = TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: "abc123".to_string(),
        exchange: Exchange::BinanceSpot,
    };

    storage.update_trade(&trade_data);
    let trade_snapshot = storage
        .get_trade_snapshot("BTCUSDT", &Exchange::BinanceSpot)
        .unwrap();
    assert_eq!(trade_snapshot.symbol, "BTCUSDT");
    assert_eq!(trade_snapshot.price, 45000.5);
}

#[test]
fn test_concurrent_atomic_operations() {
    let orderbook = Arc::new(AtomicOrderBook::new(
        "BTCUSDT".to_string(),
        Exchange::BinanceSpot,
    ));
    let mut handles = vec![];

    // Spawn multiple threads to update the orderbook concurrently
    for i in 0..10 {
        let orderbook_clone = Arc::clone(&orderbook);
        let handle = thread::spawn(move || {
            let data = OrderBookData {
                symbol: "BTCUSDT".to_string(),
                timestamp: 1640995200000 + i as i64,
                bids: vec![(45000.0 + i as f64, 1.5)],
                asks: vec![(45001.0 + i as f64, 1.2)],
                sequence: 12345 + i as u64,
                exchange: Exchange::BinanceSpot,
            };
            orderbook_clone.update_from_data(&data);
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify that the orderbook was updated (exact values may vary due to concurrency)
    let snapshot = orderbook.to_snapshot();
    assert!(snapshot.timestamp >= 1640995200000);
    assert!(snapshot.sequence >= 12345);
}

#[test]
fn test_price_quantity_conversion_functions() {
    let price = 45000.12345678;
    let atomic_price = price_to_atomic(price);
    let converted_back = atomic_to_price(atomic_price);
    assert!((price - converted_back).abs() < 0.00000001_f64);

    let quantity = 1.23456789;
    let atomic_quantity = quantity_to_atomic(quantity);
    let converted_back_qty = atomic_to_quantity(atomic_quantity);
    assert!((quantity - converted_back_qty).abs() < 0.00000001_f64);
}

#[test]
fn test_conversion_from_orderbook_data() {
    let data = OrderBookData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        bids: vec![(45000.0, 1.5), (44999.0, 2.0)],
        asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
        sequence: 12345,
        exchange: Exchange::BinanceSpot,
    };

    let snapshot = OrderBookSnapshot::from(&data);
    assert_eq!(snapshot.symbol, "BTCUSDT");
    assert_eq!(snapshot.best_bid_price, 45000.0);
    assert_eq!(snapshot.best_ask_price, 45001.0);
    assert_eq!(snapshot.sequence, 12345);
}

#[test]
fn test_conversion_from_trade_data() {
    let data = TradeData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: "abc123".to_string(),
        exchange: Exchange::BinanceSpot,
    };

    let snapshot = TradeSnapshot::from(&data);
    assert_eq!(snapshot.symbol, "BTCUSDT");
    assert_eq!(snapshot.price, 45000.5);
    assert_eq!(snapshot.quantity, 0.1);
    assert_eq!(snapshot.side, "buy");
}

#[test]
fn test_high_frequency_storage_symbols() {
    let storage = HighFrequencyStorage::new();

    let orderbook_data = OrderBookData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        bids: vec![(45000.0, 1.5)],
        asks: vec![(45001.0, 1.2)],
        sequence: 12345,
        exchange: Exchange::BinanceSpot,
    };

    let trade_data = TradeData {
        symbol: "ETHUSDT".to_string(),
        timestamp: 1640995200000,
        price: 3000.0,
        quantity: 1.0,
        side: TradeSide::Buy,
        trade_id: "eth123".to_string(),
        exchange: Exchange::BinanceSpot,
    };

    storage.update_orderbook(&orderbook_data);
    storage.update_trade(&trade_data);

    let orderbook_symbols = storage.get_orderbook_symbols();
    let trade_symbols = storage.get_trade_symbols();

    assert!(orderbook_symbols.contains(&format!("{}:{}", Exchange::BinanceSpot, "BTCUSDT")));
    assert!(trade_symbols.contains(&format!("{}:{}", Exchange::BinanceSpot, "ETHUSDT")));
}

#[test]
fn test_high_frequency_storage_remove_symbol() {
    let storage = HighFrequencyStorage::new();

    let exchange = Exchange::BinanceFutures;
    let symbol = "ETHUSDC";

    let orderbook_data = OrderBookData {
        symbol: symbol.to_string(),
        timestamp: 1640995200000,
        bids: vec![(2500.0, 3.0)],
        asks: vec![(2500.5, 1.0)],
        sequence: 42,
        exchange: exchange.clone(),
    };

    let trade_data = TradeData {
        symbol: symbol.to_string(),
        timestamp: 1640995200500,
        price: 2500.25,
        quantity: 0.5,
        side: TradeSide::Sell,
        trade_id: "ethusdc-trade-1".to_string(),
        exchange: exchange.clone(),
    };

    storage.update_orderbook(&orderbook_data);
    storage.update_trade(&trade_data);

    assert!(storage.get_orderbook_snapshot(symbol, &exchange).is_some());
    assert!(storage.get_trade_snapshot(symbol, &exchange).is_some());

    assert!(storage.remove_symbol(symbol, &exchange));

    assert!(storage.get_orderbook_snapshot(symbol, &exchange).is_none());
    assert!(storage.get_trade_snapshot(symbol, &exchange).is_none());

    let orderbook_symbols = storage.get_orderbook_symbols();
    let trade_symbols = storage.get_trade_symbols();

    let composite_key = format!("{exchange}:{symbol}");
    assert!(!orderbook_symbols.contains(&composite_key));
    assert!(!trade_symbols.contains(&composite_key));
}

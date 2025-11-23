use raven::common::db::influx_client::{
    create_candle_datapoint, create_funding_rate_datapoint, create_orderbook_datapoint,
    create_trade_datapoint, create_wallet_update_datapoint,
};
use raven::server::data_engine::storage::{
    CandleData, FundingRateData, OrderBookLevel, OrderBookSnapshot,
};
use raven::server::data_engine::storage::{TradeSide, TradeSnapshot};
use raven::server::exchanges::types::Exchange;

#[test]
fn test_datapoint_creation_orderbook() {
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

    let datapoint = create_orderbook_datapoint(&snapshot).unwrap();
    assert!(format!("{datapoint:?}").contains("orderbook"));
}

#[test]
fn test_datapoint_creation_trade() {
    let snapshot = TradeSnapshot {
        symbol: "BTCUSDT".to_string(),
        exchange: Exchange::BinanceSpot,
        timestamp: 1640995200000,
        price: 45000.5,
        quantity: 0.1,
        side: TradeSide::Buy,
        trade_id: 123456,
    };

    let datapoint = create_trade_datapoint(&snapshot).unwrap();
    assert!(format!("{datapoint:?}").contains("trades"));
}

#[test]
fn test_datapoint_creation_candle() {
    let candle = CandleData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        open: 45000.0,
        high: 45100.0,
        low: 44900.0,
        close: 45050.0,
        volume: 150.5,
        interval: "1m".to_string(),
        exchange: Exchange::BinanceSpot,
    };

    let datapoint = create_candle_datapoint(&candle).unwrap();
    assert!(format!("{datapoint:?}").contains("candles"));
}

#[test]
fn test_datapoint_creation_funding_rate() {
    let funding = FundingRateData {
        symbol: "BTCUSDT".to_string(),
        timestamp: 1640995200000,
        rate: 0.0001,
        next_funding_time: 1640995200000 + 28800000,
        exchange: Exchange::BinanceSpot,
    };

    let datapoint = create_funding_rate_datapoint(&funding).unwrap();
    assert!(format!("{datapoint:?}").contains("funding_rates"));
}

#[test]
fn test_datapoint_creation_wallet_update() {
    let datapoint =
        create_wallet_update_datapoint("user123", "BTC", 1.5, 0.1, 1640995200000).unwrap();

    assert!(format!("{datapoint:?}").contains("wallet_updates"));
}

use super::*;
use crate::exchanges::binance::app::BinanceFuturesTrade;
use crate::exchanges::websocket::WebSocketParser;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_binance_websocket_60_second_integration() {
    // Simple 60-second WebSocket integration test
    let symbol = "btcusdt".to_string();

    println!("🚀 Starting 60-second Binance Futures WebSocket test...");

    let mut client = BinanceFuturesTrade::new(symbol.clone()).expect("Failed to create client");
    let mut rx = client
        .start_streaming()
        .await
        .expect("Failed to start streaming");

    let start_time = std::time::Instant::now();
    let mut message_count = 0;
    let mut connection_confirmed = false;

    // Run for 60 seconds or until we get enough messages
    while start_time.elapsed() < Duration::from_secs(60) {
        match timeout(Duration::from_secs(1), rx.recv()).await {
            Ok(Some(msg)) => {
                if !connection_confirmed {
                    println!("✅ Connection confirmed!");
                    connection_confirmed = true;
                }

                message_count += 1;

                // Basic validation
                assert_eq!(
                    msg.exchange,
                    crate::exchanges::types::Exchange::BinanceFutures
                );

                if let crate::exchanges::types::MarketData::FutureTrade { price, .. } = &msg.data {
                    assert!(*price > 0.0);

                    if message_count % 10 == 0 {
                        println!("📊 Message #{message_count}: ${price:.2}");
                    }
                }

                // Early exit if we get enough data
                if message_count >= 20 {
                    println!(
                        "🎉 Success! Received {} messages in {:.1}s",
                        message_count,
                        start_time.elapsed().as_secs_f64()
                    );
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => {
                // Timeout - continue
                if start_time.elapsed().as_secs() % 15 == 0 {
                    println!(
                        "⏳ {}s elapsed, {} messages...",
                        start_time.elapsed().as_secs(),
                        message_count
                    );
                }
            }
        }
    }

    // Results
    let elapsed = start_time.elapsed();
    println!(
        "\n📈 Results: {:.1}s, {} messages",
        elapsed.as_secs_f64(),
        message_count
    );

    if connection_confirmed && message_count > 0 {
        println!("✅ TEST PASSED!");
    } else {
        panic!("❌ TEST FAILED - No connection or data");
    }
}

#[test]
fn test_binance_subscription_ticker() {
    let parser = BinanceSpotParser::new();
    let subscription = SubscriptionRequest {
        exchange: Exchange::BinanceSpot,
        symbol: "BTCUSDT".to_string(),
        data_type: DataType::Ticker,
    };

    let result = parser.create_subscription_message(&subscription).unwrap();
    let expected = r#"{"id":1,"method":"SUBSCRIBE","params":["btcusdt@ticker"]}"#;

    assert_eq!(result, expected);
}

#[test]
fn test_binance_subscription_orderbook() {
    let parser = BinanceSpotParser::new();
    let subscription = SubscriptionRequest {
        exchange: Exchange::BinanceSpot,
        symbol: "ETHUSDT".to_string(),
        data_type: DataType::OrderBook,
    };

    let result = parser.create_subscription_message(&subscription).unwrap();
    let expected = r#"{"id":1,"method":"SUBSCRIBE","params":["ethusdt@depth20@100ms"]}"#;

    assert_eq!(result, expected);
}

#[test]
fn test_binance_subscription_spot_trade() {
    let parser = BinanceSpotParser::new();
    let subscription = SubscriptionRequest {
        exchange: Exchange::BinanceSpot,
        symbol: "ADAUSDT".to_string(),
        data_type: DataType::SpotTrade,
    };

    let result = parser.create_subscription_message(&subscription).unwrap();
    let expected = r#"{"id":1,"method":"SUBSCRIBE","params":["adausdt@trade"]}"#;

    assert_eq!(result, expected);
}

#[test]
fn test_binance_parse_ticker() {
    let parser = BinanceSpotParser::new();
    let message = r#"{
            "stream": "btcusdt@ticker",
            "data": {
                "s": "BTCUSDT",
                "c": "45000.50",
                "w": "1234.56"
            }
        }"#;

    let result = parser.parse_ticker(message).unwrap().unwrap();

    assert_eq!(result.exchange, Exchange::BinanceSpot);
    assert_eq!(result.symbol, "BTCUSDT");

    match result.data {
        MarketData::Ticker {
            price,
            weighted_average_price,
        } => {
            assert_eq!(price, 45000.50);
            assert_eq!(weighted_average_price, 1234.56);
        }
        _ => panic!("Expected Ticker data"),
    }
}

#[test]
fn test_binance_parse_orderbook() {
    let parser = BinanceSpotParser::new();
    let message = r#"{
            "stream": "btcusdt@depth20@100ms",
            "data": {
                "s": "BTCUSDT",
                "bids": [["44900.00", "1.5"], ["44899.00", "2.0"]],
                "asks": [["45100.00", "1.2"], ["45101.00", "0.8"]]
            }
        }"#;

    let result = parser.parse_orderbook(message).unwrap().unwrap();

    assert_eq!(result.exchange, Exchange::BinanceSpot);
    assert_eq!(result.symbol, "BTCUSDT");

    match result.data {
        MarketData::OrderBook { bids, asks } => {
            assert_eq!(bids.len(), 2);
            assert_eq!(asks.len(), 2);
            assert_eq!(bids[0], (44900.00, 1.5));
            assert_eq!(asks[0], (45100.00, 1.2));
        }
        _ => panic!("Expected OrderBook data"),
    }
}

#[test]
fn test_binance_parse_spot_trade() {
    let parser = BinanceSpotParser::new();
    let message = r#"{
            "stream": "btcusdt@trade",
            "data": {
                "s": "btcusdt",
                "p": "45000.00",
                "q": "0.1",
                "t": 12345,
                "m": false
            }
        }"#;

    let result = parser.parse_spot_trade(message).unwrap().unwrap();

    assert_eq!(result.exchange, Exchange::BinanceSpot);
    assert_eq!(result.symbol, "BTCUSDT");

    match result.data {
        MarketData::SpotTrade {
            price,
            size,
            side,
            trade_id,
        } => {
            assert_eq!(price, 45000.00);
            assert_eq!(size, 0.1);
            assert!(matches!(side, TradeSide::Buy));
            assert_eq!(trade_id, "12345");
        }
        _ => panic!("Expected SpotTrade data"),
    }
}

#[test]
fn test_binance_parse_future_trade() {
    let parser = BinanceSpotParser::new();
    let message = r#"{
            "stream": "btcusdt@aggTrade",
            "data": {
                "s": "BTCUSDT",
                "p": "45000.00",
                "q": "0.5",
                "a": 67890,
                "m": true
            }
        }"#;

    let result = parser.parse_future_trade(message).unwrap().unwrap();

    assert_eq!(result.exchange, Exchange::BinanceSpot);
    assert_eq!(result.symbol, "BTCUSDT");

    match result.data {
        MarketData::FutureTrade {
            price,
            size,
            side,
            trade_id,
            contract_type,
            funding_ratio,
        } => {
            assert_eq!(price, 45000.00);
            assert_eq!(size, 0.5);
            assert!(matches!(side, TradeSide::Sell));
            assert_eq!(trade_id, "67890");
            assert_eq!(contract_type, "PERPETUAL");
            assert!(funding_ratio.is_none());
        }
        _ => panic!("Expected FutureTrade data"),
    }
}

#[test]
fn test_binance_parse_candle() {
    let parser = BinanceSpotParser::new();
    let message = r#"{
            "stream": "btcusdt@kline_1m",
            "data": {
                "k": {
                    "s": "BTCUSDT",
                    "o": "44900.00",
                    "h": "45100.00",
                    "l": "44800.00",
                    "c": "45000.00",
                    "v": "100.5",
                    "t": 1640995200000,
                    "i": "1m"
                }
            }
        }"#;

    let result = parser.parse_candle(message).unwrap().unwrap();

    assert_eq!(result.exchange, Exchange::BinanceSpot);
    assert_eq!(result.symbol, "BTCUSDT");

    match result.data {
        MarketData::Candle {
            open,
            high,
            low,
            close,
            volume,
            timestamp,
            interval,
        } => {
            assert_eq!(open, 44900.00);
            assert_eq!(high, 45100.00);
            assert_eq!(low, 44800.00);
            assert_eq!(close, 45000.00);
            assert_eq!(volume, 100.5);
            assert_eq!(timestamp, 1640995200000);
            assert_eq!(interval, "1m");
        }
        _ => panic!("Expected Candle data"),
    }
}

#[test]
fn test_binance_parse_invalid_message() {
    let parser = BinanceSpotParser::new();
    let message = r#"{"invalid": "json"}"#;

    let result = parser.parse_ticker(message).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_binance_spot_exchange() {
    let parser = BinanceSpotParser::new();
    assert_eq!(parser.exchange(), Exchange::BinanceSpot);
}

#[test]
fn test_binance_futures_exchange() {
    let parser = BinanceFuturesParser::new();
    assert_eq!(parser.exchange(), Exchange::BinanceFutures);
}

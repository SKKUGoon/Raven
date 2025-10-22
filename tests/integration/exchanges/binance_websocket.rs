use raven::exchanges::binance::app::futures::orderbook::BinanceFuturesOrderbook;
use raven::exchanges::binance::app::BinanceFuturesTrade;
use raven::exchanges::types::{Exchange, MarketData};
use std::env;
use std::time::Duration;
use tokio::time::timeout;

fn network_tests_enabled() -> bool {
    env::var("RAVEN_ENABLE_NETWORK_TESTS").unwrap_or_default() == "1"
}

#[tokio::test]
async fn test_binance_websocket_60_second_integration() {
    if !network_tests_enabled() {
        println!(
            "skipping test_binance_websocket_60_second_integration - set RAVEN_ENABLE_NETWORK_TESTS=1 to enable network integration tests"
        );
        return;
    }

    let symbol = "btcusdt".to_string();
    println!("▶ Starting 60-second Binance Futures WebSocket test...");

    let mut client = BinanceFuturesTrade::new(symbol.clone()).expect("Failed to create client");
    let mut rx = client
        .start_streaming()
        .await
        .expect("Failed to start streaming");

    let start_time = std::time::Instant::now();
    let mut message_count = 0;
    let mut connection_confirmed = false;

    while start_time.elapsed() < Duration::from_secs(60) {
        match timeout(Duration::from_secs(1), rx.recv()).await {
            Ok(Some(msg)) => {
                if !connection_confirmed {
                    println!("✓ Connection confirmed!");
                    connection_confirmed = true;
                }

                message_count += 1;
                assert_eq!(msg.exchange, Exchange::BinanceFutures);

                if let MarketData::FutureTrade { price, .. } = &msg.data {
                    assert!(*price > 0.0);
                    if message_count % 10 == 0 {
                        println!("◉ Message #{message_count}: ${price:.2}");
                    }
                }

                if message_count >= 20 {
                    println!(
                        "✓ Success! Received {} messages in {:.1}s",
                        message_count,
                        start_time.elapsed().as_secs_f64()
                    );
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => {
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

    let elapsed = start_time.elapsed();
    println!(
        "\n↗ Results: {:.1}s, {} messages",
        elapsed.as_secs_f64(),
        message_count
    );

    if !(connection_confirmed && message_count > 0) {
        panic!("✗ TEST FAILED - No connection or data");
    }
}

#[tokio::test]
async fn test_binance_futures_orderbook_live() {
    if !network_tests_enabled() {
        println!(
            "skipping test_binance_futures_orderbook_live - set RAVEN_ENABLE_NETWORK_TESTS=1 to enable network integration tests"
        );
        return;
    }

    println!("▶ Testing Binance Futures Orderbook WebSocket...");

    let symbol = "btcusdt".to_string();
    let mut client = BinanceFuturesOrderbook::new(symbol.clone()).expect("Failed to create client");
    let mut rx = client
        .start_streaming()
        .await
        .expect("Failed to start streaming");

    let mut message_count = 0;
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < Duration::from_secs(10) && message_count < 3 {
        match timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(msg)) => {
                message_count += 1;
                println!(
                    "⟐ Message {}: Exchange={:?}, Symbol={}",
                    message_count, msg.exchange, msg.symbol
                );

                if let MarketData::OrderBook { bids, asks } = &msg.data {
                    println!(
                        "   ◉ Orderbook - Bids: {}, Asks: {}",
                        bids.len(),
                        asks.len()
                    );
                    if let Some(first_bid) = bids.first() {
                        println!("   $ Best bid: {first_bid:?}");
                    }
                    if let Some(first_ask) = asks.first() {
                        println!("   $ Best ask: {first_ask:?}");
                    }
                }
            }
            Ok(None) => {
                println!("✗ Channel closed");
                break;
            }
            Err(_) => println!("⏰ Timeout waiting for message"),
        }
    }

    if message_count == 0 {
        println!("✗ No orderbook messages received in 10 seconds");
    } else {
        println!("✓ Received {message_count} orderbook messages");
    }
}

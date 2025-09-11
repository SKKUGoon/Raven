// Tests for the server module

use super::*;
use crate::database::influx_client::{InfluxClient, InfluxConfig};

#[tokio::test]
async fn test_server_creation() {
    let subscription_manager = Arc::new(SubscriptionManager::new());
    let influx_client = Arc::new(InfluxClient::new(InfluxConfig::default()));
    let hf_storage = Arc::new(HighFrequencyStorage::new());

    let server = MarketDataServer::new(subscription_manager, influx_client, hf_storage, 1000);

    assert_eq!(server.connection_manager.get_max_connections(), 1000);
    assert_eq!(server.connection_manager.get_active_connections().await, 0);
}

#[tokio::test]
async fn test_connection_management() {
    let connection_manager = ConnectionManager::new(2); // Small limit for testing

    // Test connection acceptance
    assert!(connection_manager.can_accept_connection().await);
    connection_manager
        .increment_connections(None)
        .await
        .unwrap();
    assert_eq!(connection_manager.get_active_connections().await, 1);

    assert!(connection_manager.can_accept_connection().await);
    connection_manager
        .increment_connections(None)
        .await
        .unwrap();
    assert_eq!(connection_manager.get_active_connections().await, 2);

    // Should reject when at limit
    assert!(!connection_manager.can_accept_connection().await);
    assert!(connection_manager
        .increment_connections(None)
        .await
        .is_err());

    // Test decrement
    connection_manager
        .decrement_connections(std::time::Duration::from_secs(1), None)
        .await;
    assert_eq!(connection_manager.get_active_connections().await, 1);
    assert!(connection_manager.can_accept_connection().await);
}

#[test]
fn test_data_type_conversion() {
    use crate::proto::DataType;
    use crate::subscription_manager::SubscriptionDataType;

    assert_eq!(
        MarketDataServiceImpl::convert_data_type(DataType::Orderbook),
        SubscriptionDataType::Orderbook
    );
    assert_eq!(
        MarketDataServiceImpl::convert_data_type(DataType::Trades),
        SubscriptionDataType::Trades
    );
    assert_eq!(
        MarketDataServiceImpl::convert_data_type(DataType::Candles1m),
        SubscriptionDataType::Candles1M
    );
}

#[test]
fn test_message_creation() {
    let orderbook_msg = MarketDataServiceImpl::create_orderbook_message(
        "BTCUSDT",
        45000.0,
        1.5,
        45001.0,
        1.2,
        12345,
        1640995200000,
    );

    match orderbook_msg.data {
        Some(crate::proto::market_data_message::Data::Orderbook(orderbook)) => {
            assert_eq!(orderbook.symbol, "BTCUSDT");
            assert_eq!(orderbook.sequence, 12345);
            assert_eq!(orderbook.bids.len(), 1);
            assert_eq!(orderbook.asks.len(), 1);
        }
        _ => panic!("Expected orderbook message"),
    }

    let trade_msg = MarketDataServiceImpl::create_trade_message(
        "BTCUSDT",
        45000.5,
        0.1,
        "buy",
        67890,
        1640995200000,
    );

    match trade_msg.data {
        Some(crate::proto::market_data_message::Data::Trade(trade)) => {
            assert_eq!(trade.symbol, "BTCUSDT");
            assert_eq!(trade.price, 45000.5);
            assert_eq!(trade.side, "buy");
        }
        _ => panic!("Expected trade message"),
    }
}

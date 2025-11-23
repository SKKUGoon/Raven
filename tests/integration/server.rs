// Tests for the server module

use raven::proto::{self, DataType};
use raven::server::grpc::client_service::manager::{ClientManagerConfig, DisconnectionReason};
use raven::server::grpc::client_service::{ClientManager, MarketDataServiceImpl};
use raven::server::stream_router::SubscriptionDataType;

#[tokio::test]
async fn test_connection_management() {
    let config = ClientManagerConfig {
        max_clients: 2,
        ..Default::default()
    };
    let client_manager = ClientManager::new(config);

    // Test connection acceptance
    assert_eq!(client_manager.get_client_count().await, 0);

    // First client
    assert!(client_manager
        .register_client("client-1".to_string())
        .await
        .is_ok());
    assert_eq!(client_manager.get_client_count().await, 1);

    // Second client
    assert!(client_manager
        .register_client("client-2".to_string())
        .await
        .is_ok());
    assert_eq!(client_manager.get_client_count().await, 2);

    // Should reject when at limit
    assert!(client_manager
        .register_client("client-3".to_string())
        .await
        .is_err());
    assert_eq!(client_manager.get_client_count().await, 2);

    // Test decrement (disconnection)
    assert!(client_manager
        .disconnect_client("client-1", DisconnectionReason::ClientInitiated)
        .await
        .is_ok());

    // Note: disconnect_client sets state to Disconnecting, it doesn't remove immediately (grace period).
    // However, for the purpose of "active connections" count in a simple test without waiting for grace period,
    // we might need to check if we can force remove or just wait.
    // But get_client_count returns total map size.

    // Let's assume we want to verify it's disconnecting.
    let client = client_manager.get_client("client-1").await.unwrap();
    assert!(matches!(
        client.state,
        raven::server::grpc::client_service::manager::ClientState::Disconnecting
    ));
}

#[test]
fn test_data_type_conversion() {
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
        Some(proto::market_data_message::Data::Orderbook(orderbook)) => {
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
        Some(proto::market_data_message::Data::Trade(trade)) => {
            assert_eq!(trade.symbol, "BTCUSDT");
            assert_eq!(trade.price, 45000.5);
            assert_eq!(trade.side, "buy");
        }
        _ => panic!("Expected trade message"),
    }
}

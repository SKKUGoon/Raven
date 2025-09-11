use super::*;
use tokio::time::{sleep, Duration};

fn create_test_wallet_update(user_id: &str) -> WalletUpdateData {
    WalletUpdateData {
        user_id: user_id.to_string(),
        timestamp: 1640995200000,
        balances: vec![BalanceData {
            asset: "BTC".to_string(),
            available: 1.5,
            locked: 0.1,
        }],
        exchange: "binance".to_string(),
    }
}

fn create_test_position_update(user_id: &str, symbol: &str) -> PositionUpdateData {
    PositionUpdateData {
        user_id: user_id.to_string(),
        symbol: symbol.to_string(),
        timestamp: 1640995200000,
        size: 0.1,
        entry_price: 45000.0,
        mark_price: 45100.0,
        unrealized_pnl: 10.0,
        side: "long".to_string(),
        exchange: "binance".to_string(),
    }
}

fn create_test_permissions(user_id: &str) -> ClientPermissions {
    ClientPermissions {
        user_id: user_id.to_string(),
        allowed_data_types: vec!["wallet_updates".to_string(), "position_updates".to_string()],
        access_level: AccessLevel::ReadWrite,
    }
}

#[tokio::test]
async fn test_handler_creation() {
    let handler = PrivateDataHandler::new();
    let metrics = handler.get_metrics();
    assert_eq!(metrics.get("wallet_updates_processed").unwrap(), &0);
}

#[tokio::test]
async fn test_wallet_update_ingestion() {
    let handler = PrivateDataHandler::new();
    handler.start().await.unwrap();

    let wallet_update = create_test_wallet_update("user123");
    let result = handler.ingest_wallet_update(&wallet_update).await;
    assert!(result.is_ok());

    sleep(Duration::from_millis(300)).await;

    let metrics = handler.get_metrics();
    assert_eq!(metrics.get("wallet_updates_processed").unwrap(), &1);

    handler.stop().await.unwrap();
}

#[tokio::test]
async fn test_client_permissions() {
    let handler = PrivateDataHandler::new();
    handler.start().await.unwrap();

    let user_id = "user123";
    let client_id = "client456";

    let permissions = create_test_permissions(user_id);
    handler
        .register_client(client_id, permissions)
        .await
        .unwrap();

    let wallet_update = create_test_wallet_update(user_id);
    handler.ingest_wallet_update(&wallet_update).await.unwrap();

    sleep(Duration::from_millis(300)).await;

    let wallet_data = handler.get_wallet_updates(user_id, client_id).await;
    assert!(wallet_data.is_ok());
    assert!(wallet_data.unwrap().is_some());

    let unauthorized_result = handler
        .get_wallet_updates(user_id, "unauthorized_client")
        .await;
    assert!(unauthorized_result.is_err());

    handler.stop().await.unwrap();
}

#[test]
fn test_storage_operations() {
    let storage = PrivateDataStorage::new(10);

    let wallet = create_test_wallet_update("user123");
    storage.add_wallet_update(&wallet);

    let position = create_test_position_update("user123", "BTCUSDT");
    storage.add_position_update(&position);

    let all_users = storage.get_all_user_ids();
    assert!(all_users.contains(&"user123".to_string()));
}

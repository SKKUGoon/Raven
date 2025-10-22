// Client Manager Tests - Project Raven
// "Testing the managers of our message carriers"

use raven::client_manager::{
    ClientManager, ClientManagerConfig, ClientState, ConnectionQuality, DisconnectionReason,
};
use raven::RavenError;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_client_registration() {
    let config = ClientManagerConfig::default();
    let manager = ClientManager::new(config);

    let result = manager.register_client("test-client".to_string()).await;
    assert!(result.is_ok());

    let client = manager.get_client("test-client").await;
    assert!(client.is_some());
    assert_eq!(client.unwrap().client_id, "test-client");
}

#[tokio::test]
async fn test_max_clients_limit() {
    let config = ClientManagerConfig {
        max_clients: 2,
        ..Default::default()
    };
    let manager = ClientManager::new(config);

    // Register up to the limit
    assert!(manager.register_client("client1".to_string()).await.is_ok());
    assert!(manager.register_client("client2".to_string()).await.is_ok());

    // Should fail when exceeding limit
    let result = manager.register_client("client3".to_string()).await;
    assert!(result.is_err());

    if let Err(RavenError::MaxConnectionsExceeded { current, max }) = result {
        assert_eq!(current, 2);
        assert_eq!(max, 2);
    } else {
        panic!("Expected MaxConnectionsExceeded error");
    }
}

#[tokio::test]
async fn test_heartbeat_update() {
    let config = ClientManagerConfig::default();
    let manager = ClientManager::new(config);

    manager
        .register_client("test-client".to_string())
        .await
        .unwrap();

    let result = manager.update_heartbeat("test-client").await;
    assert!(result.is_ok());

    let client = manager.get_client("test-client").await.unwrap();
    assert!(client.last_heartbeat.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn test_client_disconnection() {
    let config = ClientManagerConfig::default();
    let manager = ClientManager::new(config);

    manager
        .register_client("test-client".to_string())
        .await
        .unwrap();

    let result = manager
        .disconnect_client("test-client", DisconnectionReason::ClientInitiated)
        .await;
    assert!(result.is_ok());

    let client = manager.get_client("test-client").await.unwrap();
    assert_eq!(client.state, ClientState::Disconnecting);
}

#[tokio::test]
async fn test_client_activity_tracking() {
    let config = ClientManagerConfig::default();
    let manager = ClientManager::new(config);

    manager
        .register_client("test-client".to_string())
        .await
        .unwrap();

    manager
        .increment_messages_sent("test-client")
        .await
        .unwrap();
    manager
        .increment_messages_received("test-client")
        .await
        .unwrap();

    let client = manager.get_client("test-client").await.unwrap();
    assert_eq!(client.messages_sent, 1);
    assert_eq!(client.messages_received, 1);
}

#[tokio::test]
async fn test_client_metadata() {
    let config = ClientManagerConfig::default();
    let manager = ClientManager::new(config);

    manager
        .register_client("test-client".to_string())
        .await
        .unwrap();
    manager
        .add_client_metadata("test-client", "ip", "127.0.0.1")
        .await
        .unwrap();

    let client = manager.get_client("test-client").await.unwrap();
    assert_eq!(client.metadata.get("ip"), Some(&"127.0.0.1".to_string()));
}

#[tokio::test]
async fn test_connection_quality() {
    let mut quality = ConnectionQuality::new();

    quality.record_heartbeat();
    assert_eq!(quality.heartbeats_received, 1);
    assert_eq!(quality.stability_score, 1.0);

    quality.record_missed_heartbeat();
    assert_eq!(quality.missed_heartbeats, 1);
    assert_eq!(quality.stability_score, 0.5); // 1 success out of 2 total
}

#[tokio::test]
async fn test_client_health_check() {
    let config = ClientManagerConfig {
        health_check_interval: Duration::from_millis(100),
        ..Default::default()
    };
    let manager = ClientManager::new(config);

    manager
        .register_client("test-client".to_string())
        .await
        .unwrap();

    // Client should be healthy initially
    let client = manager.get_client("test-client").await.unwrap();
    assert!(client.is_healthy(Duration::from_millis(100)));

    // Wait for heartbeat timeout
    sleep(Duration::from_millis(150)).await;

    let client = manager.get_client("test-client").await.unwrap();
    assert!(!client.is_healthy(Duration::from_millis(100)));
}

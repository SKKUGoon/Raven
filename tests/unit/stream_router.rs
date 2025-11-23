// Stream Router Tests - Project Raven
// "Testing the central switchboard for routing real-time data streams"

use raven::proto::DataType;
use raven::server::stream_router::{
    ClientSubscription, StreamRouter, SubscriptionDataType, TopicIndex,
};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

#[tokio::test]
async fn test_subscription_creation() {
    let router = StreamRouter::new();
    let (sender, _receiver) = mpsc::unbounded_channel();

    let topics = router
        .subscribe(
            "client1".to_string(),
            vec!["BTCUSDT".to_string()],
            vec![SubscriptionDataType::Orderbook],
            HashMap::new(),
            sender,
        )
        .unwrap();

    assert!(!topics.is_empty());
    assert_eq!(router.get_active_clients().len(), 1);
    assert!(router.get_subscription("client1").is_some());
}

#[tokio::test]
async fn test_subscription_unsubscribe() {
    let router = StreamRouter::new();
    let (sender, _receiver) = mpsc::unbounded_channel();

    // Subscribe
    router
        .subscribe(
            "client1".to_string(),
            vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
            vec![
                SubscriptionDataType::Orderbook,
                SubscriptionDataType::Trades,
            ],
            HashMap::new(),
            sender,
        )
        .unwrap();

    // Partial unsubscribe
    let unsubscribed = router
        .unsubscribe(
            "client1",
            vec!["BTCUSDT".to_string()],
            vec![SubscriptionDataType::Orderbook],
        )
        .unwrap();

    assert!(!unsubscribed.is_empty());

    // Full unsubscribe
    router.unsubscribe_all("client1").unwrap();
    assert_eq!(router.get_active_clients().len(), 0);

    // Complete removal
    router.remove_client_completely("client1").unwrap();
}

#[tokio::test]
async fn test_heartbeat_management() {
    let router = StreamRouter::new();
    let (sender, _receiver) = mpsc::unbounded_channel();

    router
        .subscribe(
            "client1".to_string(),
            vec!["BTCUSDT".to_string()],
            vec![SubscriptionDataType::Orderbook],
            HashMap::new(),
            sender,
        )
        .unwrap();

    // Update heartbeat
    router.update_heartbeat("client1").unwrap();

    let subscription = router.get_subscription("client1").unwrap();
    assert!(subscription.is_alive(30_000));
}

#[tokio::test]
async fn test_topic_routing() {
    let index = TopicIndex::new();

    // Add subscriptions
    index.add_subscription("client1", vec!["BTCUSDT:Orderbook".to_string()]);
    index.add_subscription("client2", vec!["*".to_string()]);
    index.add_subscription("client3", vec!["BTCUSDT:*".to_string()]);

    // Test topic matching
    let clients = index.get_clients_for_topic("BTCUSDT:Orderbook");
    assert_eq!(clients.len(), 3); // All three should match

    let clients = index.get_clients_for_topic("ETHUSDT:Trades");
    assert_eq!(clients.len(), 1); // Only wildcard client should match

    // Remove subscription
    index.remove_client("client1");
    let clients = index.get_clients_for_topic("BTCUSDT:Orderbook");
    assert_eq!(clients.len(), 2); // Two remaining clients
}

#[test]
fn test_subscription_persistence() {
    let router = StreamRouter::new();
    let (sender1, _receiver1) = mpsc::unbounded_channel();

    // Subscribe client
    router
        .subscribe(
            "client1".to_string(),
            vec!["BTCUSDT".to_string()],
            vec![SubscriptionDataType::Orderbook],
            HashMap::new(),
            sender1,
        )
        .unwrap();

    // Check that subscription is persisted
    assert!(router.has_persisted_subscription("client1"));

    // Simulate disconnection (but keep persistence)
    router.unsubscribe_all("client1").unwrap();

    // Check that persistence is still there
    assert!(router.has_persisted_subscription("client1"));

    // Check that active subscription is gone
    assert!(router.get_subscription("client1").is_none());

    // Remove persistence completely
    router.remove_persisted_subscription("client1");
    assert!(!router.has_persisted_subscription("client1"));
}

#[test]
fn test_client_subscription_matching() {
    let (sender, _receiver) = mpsc::unbounded_channel();
    let subscription = ClientSubscription::new(
        "client1".to_string(),
        vec!["BTCUSDT".to_string()].into_iter().collect(),
        vec![SubscriptionDataType::Orderbook].into_iter().collect(),
        HashMap::new(),
        sender,
    );

    assert!(subscription.matches("BTCUSDT", &SubscriptionDataType::Orderbook));
    assert!(!subscription.matches("ETHUSDT", &SubscriptionDataType::Orderbook));
    assert!(!subscription.matches("BTCUSDT", &SubscriptionDataType::Trades));
}

#[test]
fn test_subscription_data_type_conversion() {
    let proto_type = DataType::Orderbook;
    let sub_type: SubscriptionDataType = proto_type.into();
    assert_eq!(sub_type, SubscriptionDataType::Orderbook);

    let back_to_proto: DataType = sub_type.into();
    assert_eq!(back_to_proto, DataType::Orderbook);
}

#[test]
fn test_topic_key_generation() {
    let (sender, _receiver) = mpsc::unbounded_channel();

    // Specific symbol and data type
    let subscription = ClientSubscription::new(
        "client1".to_string(),
        vec!["BTCUSDT".to_string()].into_iter().collect(),
        vec![SubscriptionDataType::Orderbook].into_iter().collect(),
        HashMap::new(),
        sender.clone(),
    );
    let topics = subscription.get_topic_keys();
    assert_eq!(topics, vec!["BTCUSDT:Orderbook"]);

    // All symbols, specific data type
    let subscription = ClientSubscription::new(
        "client2".to_string(),
        HashSet::new(),
        vec![SubscriptionDataType::Trades].into_iter().collect(),
        HashMap::new(),
        sender.clone(),
    );
    let topics = subscription.get_topic_keys();
    assert_eq!(topics, vec!["*:Trades"]);

    // Specific symbol, all data types
    let subscription = ClientSubscription::new(
        "client3".to_string(),
        vec!["ETHUSDT".to_string()].into_iter().collect(),
        HashSet::new(),
        HashMap::new(),
        sender.clone(),
    );
    let topics = subscription.get_topic_keys();
    assert_eq!(topics, vec!["ETHUSDT:*"]);

    // All symbols and data types
    let subscription = ClientSubscription::new(
        "client4".to_string(),
        HashSet::new(),
        HashSet::new(),
        HashMap::new(),
        sender,
    );
    let topics = subscription.get_topic_keys();
    assert_eq!(topics, vec!["*"]);
}

#[tokio::test]
async fn test_cleanup_dead_clients_removes_closed_channels() {
    let router = StreamRouter::new();
    let (sender, receiver) = mpsc::unbounded_channel();

    router
        .subscribe(
            "client_cleanup".to_string(),
            vec!["BTCUSDT".to_string()],
            vec![SubscriptionDataType::Orderbook],
            HashMap::new(),
            sender,
        )
        .unwrap();

    // Drop the receiver to simulate client disconnect
    drop(receiver);

    router.cleanup_dead_clients().await;

    assert!(router.get_subscription("client_cleanup").is_none());
}

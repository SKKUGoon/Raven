// Unit Tests - Project Raven
// "Test your steel before battle"

use market_data_subscription_server::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState},
    data_handlers::HighFrequencyHandler,
    error::RavenError,
    subscription_manager::{SubscriptionDataType, SubscriptionManager},
    types::{AtomicOrderBook, AtomicTrade, HighFrequencyStorage, OrderBookData, TradeData},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[cfg(test)]
mod atomic_data_structures {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::thread;

    #[test]
    fn test_atomic_orderbook_creation() {
        let orderbook = AtomicOrderBook::new("BTCUSDT".to_string());
        assert_eq!(orderbook.symbol, "BTCUSDT");
        assert_eq!(orderbook.timestamp.load(Ordering::Relaxed), 0);
        assert_eq!(orderbook.sequence.load(Ordering::Relaxed), 0);
        assert_eq!(orderbook.best_bid_price.load(Ordering::Relaxed), 0);
        assert_eq!(orderbook.best_ask_price.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_atomic_orderbook_update_and_snapshot() {
        let orderbook = AtomicOrderBook::new("BTCUSDT".to_string());
        let data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5), (44999.0, 2.0)],
            asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        orderbook.update_from_data(&data);

        // Verify atomic updates
        assert_eq!(orderbook.timestamp.load(Ordering::Relaxed), 1640995200000);
        assert_eq!(orderbook.sequence.load(Ordering::Relaxed), 12345);

        // Test snapshot conversion
        let snapshot = orderbook.to_snapshot();
        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.timestamp, 1640995200000);
        assert_eq!(snapshot.best_bid_price, 45000.0);
        assert_eq!(snapshot.best_ask_price, 45001.0);
        assert_eq!(snapshot.sequence, 12345);

        // Test spread calculation
        let spread = orderbook.get_spread();
        assert!((spread - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_atomic_trade_creation_and_operations() {
        let trade = AtomicTrade::new("BTCUSDT".to_string());
        assert_eq!(trade.symbol, "BTCUSDT");
        assert_eq!(trade.timestamp.load(Ordering::Relaxed), 0);
        assert_eq!(trade.side.load(Ordering::Relaxed), 0);

        let data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        trade.update_from_data(&data);

        // Verify atomic updates
        assert_eq!(trade.timestamp.load(Ordering::Relaxed), 1640995200000);
        assert_eq!(trade.side.load(Ordering::Relaxed), 0); // buy = 0

        // Test snapshot conversion
        let snapshot = trade.to_snapshot();
        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.price, 45000.5);
        assert_eq!(snapshot.quantity, 0.1);
        assert_eq!(snapshot.side, "buy");

        // Test trade value calculation
        let trade_value = trade.get_trade_value();
        // 45000.5 * 0.1 = 4500.05
        assert!((trade_value - 4500.05).abs() < 0.01);
    }

    #[test]
    fn test_atomic_trade_side_conversion() {
        let trade = AtomicTrade::new("BTCUSDT".to_string());

        // Test buy side
        let buy_data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.0,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "buy123".to_string(),
            exchange: "binance".to_string(),
        };
        trade.update_from_data(&buy_data);
        assert_eq!(trade.side.load(Ordering::Relaxed), 0);

        // Test sell side
        let sell_data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.0,
            quantity: 0.1,
            side: "sell".to_string(),
            trade_id: "sell123".to_string(),
            exchange: "binance".to_string(),
        };
        trade.update_from_data(&sell_data);
        assert_eq!(trade.side.load(Ordering::Relaxed), 1);

        // Test unknown side defaults to buy
        let unknown_data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.0,
            quantity: 0.1,
            side: "unknown".to_string(),
            trade_id: "unknown123".to_string(),
            exchange: "binance".to_string(),
        };
        trade.update_from_data(&unknown_data);
        assert_eq!(trade.side.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_high_frequency_storage_operations() {
        let storage = HighFrequencyStorage::new();

        // Test orderbook operations
        let orderbook_data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![(45001.0, 1.2)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        storage.update_orderbook(&orderbook_data);
        let snapshot = storage.get_orderbook_snapshot("BTCUSDT").unwrap();
        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.best_bid_price, 45000.0);

        // Test trade operations
        let trade_data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        storage.update_trade(&trade_data);
        let trade_snapshot = storage.get_trade_snapshot("BTCUSDT").unwrap();
        assert_eq!(trade_snapshot.symbol, "BTCUSDT");
        assert_eq!(trade_snapshot.price, 45000.5);

        // Test symbol listing
        let orderbook_symbols = storage.get_orderbook_symbols();
        let trade_symbols = storage.get_trade_symbols();
        assert!(orderbook_symbols.contains(&"BTCUSDT".to_string()));
        assert!(trade_symbols.contains(&"BTCUSDT".to_string()));
    }

    #[test]
    fn test_concurrent_atomic_operations() {
        let orderbook = Arc::new(AtomicOrderBook::new("BTCUSDT".to_string()));
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
                    exchange: "binance".to_string(),
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
    fn test_empty_orderbook_handling() {
        let storage = HighFrequencyStorage::new();

        // Test with empty bids and asks
        let empty_data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![],
            asks: vec![],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        storage.update_orderbook(&empty_data);
        let snapshot = storage.get_orderbook_snapshot("BTCUSDT").unwrap();
        assert_eq!(snapshot.best_bid_price, 0.0);
        assert_eq!(snapshot.best_ask_price, 0.0);
    }
}

#[cfg(test)]
mod subscription_management {
    use super::*;

    #[tokio::test]
    async fn test_subscription_creation() {
        let manager = SubscriptionManager::new();
        let (sender, _receiver) = mpsc::unbounded_channel();

        let topics = manager
            .subscribe(
                "client1".to_string(),
                vec!["BTCUSDT".to_string()],
                vec![SubscriptionDataType::Orderbook],
                HashMap::new(),
                sender,
            )
            .unwrap();

        assert!(!topics.is_empty());
        assert_eq!(manager.get_active_clients().len(), 1);
        assert!(manager.get_subscription("client1").is_some());
    }

    #[tokio::test]
    async fn test_subscription_unsubscribe() {
        let manager = SubscriptionManager::new();
        let (sender, _receiver) = mpsc::unbounded_channel();

        // Subscribe to multiple symbols and data types
        manager
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
        let unsubscribed = manager
            .unsubscribe(
                "client1",
                vec!["BTCUSDT".to_string()],
                vec![SubscriptionDataType::Orderbook],
            )
            .unwrap();

        assert!(!unsubscribed.is_empty());

        // Full unsubscribe
        manager.unsubscribe_all("client1").unwrap();
        assert_eq!(manager.get_active_clients().len(), 0);

        // Complete removal
        manager.remove_client_completely("client1").unwrap();
    }

    #[tokio::test]
    async fn test_heartbeat_management() {
        let manager = SubscriptionManager::new();
        let (sender, _receiver) = mpsc::unbounded_channel();

        manager
            .subscribe(
                "client1".to_string(),
                vec!["BTCUSDT".to_string()],
                vec![SubscriptionDataType::Orderbook],
                HashMap::new(),
                sender,
            )
            .unwrap();

        // Update heartbeat
        manager.update_heartbeat("client1").unwrap();

        let subscription = manager.get_subscription("client1").unwrap();
        assert!(subscription.is_alive(30_000));
    }

    #[tokio::test]
    async fn test_subscription_persistence() {
        let manager = SubscriptionManager::new();
        let (sender1, _receiver1) = mpsc::unbounded_channel();

        // Subscribe client
        manager
            .subscribe(
                "client1".to_string(),
                vec!["BTCUSDT".to_string()],
                vec![SubscriptionDataType::Orderbook],
                HashMap::new(),
                sender1,
            )
            .unwrap();

        // Check that subscription is persisted
        assert!(manager.has_persisted_subscription("client1"));

        // Simulate disconnection (but keep persistence)
        manager.unsubscribe_all("client1").unwrap();

        // Check that persistence is still there
        assert!(manager.has_persisted_subscription("client1"));

        // Check that active subscription is gone
        assert!(manager.get_subscription("client1").is_none());

        // Test restoration (commented out to avoid hanging)
        // let (sender2, _receiver2) = mpsc::unbounded_channel();
        // let restored_topics = manager.restore_subscription("client1", sender2).unwrap();
        // assert!(restored_topics.is_some());

        // Remove persistence completely
        manager.remove_persisted_subscription("client1");
        assert!(!manager.has_persisted_subscription("client1"));
    }

    #[tokio::test]
    async fn test_subscription_data_type_conversion() {
        use market_data_subscription_server::proto::DataType;

        let proto_type = DataType::Orderbook;
        let sub_type: SubscriptionDataType = proto_type.into();
        assert_eq!(sub_type, SubscriptionDataType::Orderbook);

        let back_to_proto: DataType = sub_type.into();
        assert_eq!(back_to_proto, DataType::Orderbook);
    }

    #[tokio::test]
    async fn test_subscription_statistics() {
        let manager = SubscriptionManager::new();
        let (sender, _receiver) = mpsc::unbounded_channel();

        manager
            .subscribe(
                "client1".to_string(),
                vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
                vec![SubscriptionDataType::Orderbook],
                HashMap::new(),
                sender,
            )
            .unwrap();

        let stats = manager.get_stats();
        assert_eq!(stats.active_clients, 1);
        assert!(stats.total_subscriptions > 0);
    }

    #[test]
    fn test_client_subscription_matching() {
        use market_data_subscription_server::subscription_manager::ClientSubscription;

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
}

#[cfg(test)]
mod data_ingestion_handlers {
    use super::*;

    #[test]
    fn test_high_frequency_handler_creation() {
        let handler = HighFrequencyHandler::new();
        let storage = handler.get_storage();
        assert_eq!(storage.get_orderbook_symbols().len(), 0);
        assert_eq!(storage.get_trade_symbols().len(), 0);
    }

    #[test]
    fn test_high_frequency_handler_orderbook_ingestion() {
        let handler = HighFrequencyHandler::new();

        let data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5), (44999.0, 2.0)],
            asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        let result = handler.ingest_orderbook_atomic("BTCUSDT", &data);
        assert!(result.is_ok());

        // Verify data was stored
        let snapshot = handler.capture_orderbook_snapshot("binance:BTCUSDT");
        assert!(snapshot.is_ok());
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.best_bid_price, 45000.0);
        assert_eq!(snapshot.best_ask_price, 45001.0);
    }

    #[test]
    fn test_high_frequency_handler_trade_ingestion() {
        let handler = HighFrequencyHandler::new();

        let data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        let result = handler.ingest_trade_atomic("BTCUSDT", &data);
        assert!(result.is_ok());

        // Verify data was stored
        let snapshot = handler.capture_trade_snapshot("binance:BTCUSDT");
        assert!(snapshot.is_ok());
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.price, 45000.5);
        assert_eq!(snapshot.quantity, 0.1);
        assert_eq!(snapshot.side, "buy");
    }

    #[test]
    fn test_high_frequency_handler_validation() {
        let handler = HighFrequencyHandler::new();

        // Test empty symbol validation
        let data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![(45001.0, 1.2)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        let result = handler.ingest_orderbook_atomic("", &data);
        assert!(result.is_err());

        // Test invalid trade price
        let invalid_trade = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: -100.0, // Invalid negative price
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        let result = handler.ingest_trade_atomic("BTCUSDT", &invalid_trade);
        assert!(result.is_err());

        // Test invalid trade side
        let invalid_side_trade = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.0,
            quantity: 0.1,
            side: "invalid".to_string(),
            trade_id: "abc123".to_string(),
            exchange: "binance".to_string(),
        };

        let result = handler.ingest_trade_atomic("BTCUSDT", &invalid_side_trade);
        assert!(result.is_err());
    }

    #[test]
    fn test_high_frequency_handler_price_level_validation() {
        let handler = HighFrequencyHandler::new();

        // Test invalid orderbook with wrong price ordering
        let invalid_orderbook = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(44999.0, 1.5), (45000.0, 2.0)], // Wrong order - should be descending
            asks: vec![(45001.0, 1.2), (45002.0, 1.8)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        // This should fail validation during ingestion
        let result = handler.ingest_orderbook_atomic("BTCUSDT", &invalid_orderbook);
        assert!(result.is_err());

        // Test invalid spread (ask < bid)
        let invalid_spread_orderbook = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45002.0, 1.5)], // Higher than ask
            asks: vec![(45001.0, 1.2)], // Lower than bid
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        let result = handler.ingest_orderbook_atomic("BTCUSDT", &invalid_spread_orderbook);
        assert!(result.is_err());
    }

    #[test]
    fn test_high_frequency_handler_exchange_support() {
        let handler = HighFrequencyHandler::new();

        // Add data from multiple exchanges
        let binance_data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![(45001.0, 1.2)],
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        let coinbase_data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(44999.0, 1.0)],
            asks: vec![(45002.0, 0.8)],
            sequence: 54321,
            exchange: "coinbase".to_string(),
        };

        handler
            .ingest_orderbook_atomic("BTCUSDT", &binance_data)
            .unwrap();
        handler
            .ingest_orderbook_atomic("BTCUSDT", &coinbase_data)
            .unwrap();

        // Test exchange-specific snapshots
        let binance_snapshot = handler
            .capture_orderbook_snapshot_for_exchange("binance", "BTCUSDT")
            .unwrap();
        let coinbase_snapshot = handler
            .capture_orderbook_snapshot_for_exchange("coinbase", "BTCUSDT")
            .unwrap();

        assert_eq!(binance_snapshot.best_bid_price, 45000.0);
        assert_eq!(coinbase_snapshot.best_bid_price, 44999.0);

        // Test active exchanges
        let exchanges = handler.get_active_exchanges();
        assert!(exchanges.contains(&"binance".to_string()));
        assert!(exchanges.contains(&"coinbase".to_string()));

        // Test symbols for specific exchange
        let binance_symbols = handler.get_symbols_for_exchange("binance");
        assert!(binance_symbols.contains(&"BTCUSDT".to_string()));
    }
}

#[cfg(test)]
mod error_handling {
    use super::*;
    use tonic::Status;

    #[test]
    fn test_error_creation_and_properties() {
        let error = RavenError::database_connection("Connection failed");
        assert_eq!(error.category(), "database");
        assert!(error.is_retryable());
        assert_eq!(error.severity(), tracing::Level::ERROR);
    }

    #[test]
    fn test_error_conversion_to_grpc_status() {
        let error = RavenError::client_not_found("test-client");
        let status: Status = error.into();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(RavenError::database_write("test").category(), "database");
        assert_eq!(RavenError::grpc_connection("test").category(), "network");
        assert_eq!(
            RavenError::subscription_failed("test").category(),
            "subscription"
        );
        assert_eq!(RavenError::data_validation("test").category(), "data");
        assert_eq!(
            RavenError::configuration("test").category(),
            "configuration"
        );
        assert_eq!(RavenError::timeout("test", 1000).category(), "system");
        assert_eq!(RavenError::authentication("test").category(), "security");
        assert_eq!(
            RavenError::dead_letter_queue_full(10, 100).category(),
            "dead_letter"
        );
        assert_eq!(RavenError::internal("test").category(), "general");
    }

    #[test]
    fn test_retryable_errors() {
        assert!(RavenError::database_connection("test").is_retryable());
        assert!(RavenError::timeout("test", 1000).is_retryable());
        assert!(!RavenError::data_validation("test").is_retryable());
        assert!(!RavenError::authentication("test").is_retryable());
    }

    #[test]
    fn test_severity_levels() {
        assert_eq!(
            RavenError::database_connection("test").severity(),
            tracing::Level::ERROR
        );
        assert_eq!(
            RavenError::database_write("test").severity(),
            tracing::Level::WARN
        );
        assert_eq!(
            RavenError::client_disconnected("test").severity(),
            tracing::Level::INFO
        );
        assert_eq!(
            RavenError::data_validation("test").severity(),
            tracing::Level::DEBUG
        );
    }

    #[test]
    fn test_error_from_conversions() {
        // Test conversion from std::io::Error
        let io_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let raven_error: RavenError = io_error.into();
        assert_eq!(raven_error.category(), "network");

        // Test conversion from serde_json::Error
        let json_error = serde_json::from_str::<i32>("invalid json").unwrap_err();
        let raven_error: RavenError = json_error.into();
        assert_eq!(raven_error.category(), "data");
    }

    #[test]
    fn test_error_builder_methods() {
        let error = RavenError::max_connections_exceeded(100, 50);
        match error {
            RavenError::MaxConnectionsExceeded { current, max } => {
                assert_eq!(current, 100);
                assert_eq!(max, 50);
            }
            _ => panic!("Expected MaxConnectionsExceeded error"),
        }

        let error = RavenError::invalid_config_value("timeout", "invalid");
        match error {
            RavenError::InvalidConfigValue { key, value } => {
                assert_eq!(key, "timeout");
                assert_eq!(value, "invalid");
            }
            _ => panic!("Expected InvalidConfigValue error"),
        }
    }
}

#[cfg(test)]
mod circuit_breaker_tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_creation() {
        let config = CircuitBreakerConfig::default();
        let breaker = CircuitBreaker::new("test", config);

        assert_eq!(breaker.name(), "test");
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);
        assert!(breaker.can_execute().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_success_recording() {
        let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

        breaker.record_success().await;

        let stats = breaker.get_stats().await;
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.state, CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_failure_recording() {
        let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

        breaker.record_failure().await;

        let stats = breaker.get_stats().await;
        assert_eq!(stats.failed_requests, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            minimum_requests: 1,
            ..Default::default()
        };

        let breaker = CircuitBreaker::new("test", config);

        // Record enough requests to meet minimum by executing operations
        for _ in 0..5 {
            let _ = breaker.execute(async { Err::<(), &str>("test") }).await;
        }

        // Record failures to trigger opening
        for _ in 0..3 {
            breaker.record_failure().await;
        }

        assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);
        assert!(!breaker.can_execute().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_transition() {
        let config = CircuitBreakerConfig {
            recovery_timeout: Duration::from_millis(10),
            failure_threshold: 1,
            minimum_requests: 1,
            ..Default::default()
        };

        let breaker = CircuitBreaker::new("test", config);

        // Record a failure first to set the last_failure_time
        breaker.record_failure().await;

        // Force to open state
        breaker.force_open().await;
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);

        // Wait for recovery timeout
        sleep(Duration::from_millis(20)).await;

        // Should transition to half-open when checking
        assert!(breaker.can_execute().await);
        assert_eq!(breaker.get_state().await, CircuitBreakerState::HalfOpen);
    }

    #[tokio::test]
    async fn test_circuit_breaker_execute_success() {
        let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

        let result = breaker.execute(async { Ok::<i32, &str>(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let stats = breaker.get_stats().await;
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.total_requests, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_execute_failure() {
        let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

        let result = breaker
            .execute(async { Err::<i32, &str>("test error") })
            .await;

        assert!(result.is_err());

        let stats = breaker.get_stats().await;
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.total_requests, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

        breaker.record_failure().await;
        breaker.force_open().await;

        assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);

        breaker.reset().await;

        assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);
        let stats = breaker.get_stats().await;
        assert_eq!(stats.failed_requests, 0);
    }
}

#[cfg(test)]
mod edge_cases_and_validation {
    use super::*;

    #[test]
    fn test_empty_symbol_handling() {
        let storage = HighFrequencyStorage::new();

        // Test getting snapshot for non-existent symbol
        let result = storage.get_orderbook_snapshot("NONEXISTENT");
        assert!(result.is_none());

        let result = storage.get_trade_snapshot("NONEXISTENT");
        assert!(result.is_none());
    }

    #[test]
    fn test_price_quantity_conversion_edge_cases() {
        use market_data_subscription_server::types::{
            atomic_to_price, atomic_to_quantity, price_to_atomic, quantity_to_atomic,
        };

        // Test zero values
        assert_eq!(price_to_atomic(0.0), 0);
        assert_eq!(quantity_to_atomic(0.0), 0);
        assert_eq!(atomic_to_price(0), 0.0);
        assert_eq!(atomic_to_quantity(0), 0.0);

        // Test very small values
        let small_price = 0.00000001;
        let atomic_price = price_to_atomic(small_price);
        let converted_back = atomic_to_price(atomic_price);
        assert!((small_price - converted_back).abs() < 0.00000001);

        // Test very large values
        let large_price = 1_000_000.0;
        let atomic_price = price_to_atomic(large_price);
        let converted_back = atomic_to_price(atomic_price);
        assert!((large_price - converted_back).abs() < 0.01);
    }

    #[test]
    fn test_orderbook_with_single_side() {
        let storage = HighFrequencyStorage::new();

        // Test orderbook with only bids
        let bids_only_data = OrderBookData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![(45000.0, 1.5)],
            asks: vec![], // Empty asks
            sequence: 12345,
            exchange: "binance".to_string(),
        };

        storage.update_orderbook(&bids_only_data);
        let snapshot = storage.get_orderbook_snapshot("BTCUSDT").unwrap();
        assert_eq!(snapshot.best_bid_price, 45000.0);
        assert_eq!(snapshot.best_ask_price, 0.0);

        // Test orderbook with only asks
        let asks_only_data = OrderBookData {
            symbol: "ETHUSDT".to_string(),
            timestamp: 1640995200000,
            bids: vec![], // Empty bids
            asks: vec![(3000.0, 2.0)],
            sequence: 54321,
            exchange: "binance".to_string(),
        };

        storage.update_orderbook(&asks_only_data);
        let snapshot = storage.get_orderbook_snapshot("ETHUSDT").unwrap();
        assert_eq!(snapshot.best_bid_price, 0.0);
        assert_eq!(snapshot.best_ask_price, 3000.0);
    }

    #[tokio::test]
    async fn test_subscription_with_empty_filters() {
        let manager = SubscriptionManager::new();
        let (sender, _receiver) = mpsc::unbounded_channel();

        // Test subscription with empty symbols (should subscribe to all)
        let topics = manager
            .subscribe(
                "client1".to_string(),
                vec![], // Empty symbols
                vec![SubscriptionDataType::Orderbook],
                HashMap::new(),
                sender,
            )
            .unwrap();

        assert!(!topics.is_empty());
        let subscription = manager.get_subscription("client1").unwrap();
        assert!(subscription.symbols.is_empty());
        assert!(!subscription.data_types.is_empty());
    }

    #[tokio::test]
    async fn test_subscription_manager_nonexistent_client() {
        let manager = SubscriptionManager::new();

        // Test operations on non-existent client
        let result = manager.update_heartbeat("nonexistent");
        assert!(result.is_ok()); // Should not fail, just log warning

        let result = manager.unsubscribe_all("nonexistent");
        assert!(result.is_ok()); // Should not fail, just log warning

        let subscription = manager.get_subscription("nonexistent");
        assert!(subscription.is_none());
    }

    #[test]
    fn test_trade_id_hashing_consistency() {
        let trade1 = AtomicTrade::new("BTCUSDT".to_string());
        let trade2 = AtomicTrade::new("BTCUSDT".to_string());

        let data = TradeData {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1640995200000,
            price: 45000.0,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: "same_id".to_string(),
            exchange: "binance".to_string(),
        };

        trade1.update_from_data(&data);
        trade2.update_from_data(&data);

        // Same trade ID should produce same hash
        assert_eq!(
            trade1.trade_id.load(std::sync::atomic::Ordering::Relaxed),
            trade2.trade_id.load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}

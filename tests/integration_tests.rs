// Integration Tests - Project Raven
// "The great trial by combat - testing the complete system"

use futures_util::StreamExt;
use market_data_subscription_server::{
    citadel::{Citadel, CitadelConfig},
    config::{DatabaseConfig, ServerConfig},
    database::influx_client::{InfluxClient, InfluxConfig},
    monitoring::MetricsCollector,
    proto::{
        market_data_service_client::MarketDataServiceClient, DataType, HistoricalDataRequest,
        SubscribeRequest, SubscriptionRequest, UnsubscribeRequest,
    },
    server::MarketDataServer,
    subscription_manager::SubscriptionManager,
    types::{HighFrequencyStorage, OrderBookData, TradeData},
};
use rand::Rng;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tempfile::TempDir;
// Note: Using mock InfluxDB for testing instead of real containers
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};
use tonic::{transport::Channel, Request};
use uuid::Uuid;

/// Test configuration for integration tests
struct TestConfig {
    pub server_port: u16,
    pub influx_port: u16,
    pub _temp_dir: TempDir,
}

impl TestConfig {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            server_port: rng.gen_range(50000..60000),
            influx_port: rng.gen_range(8000..9000),
            _temp_dir: TempDir::new().expect("Failed to create temp directory"),
        }
    }

    fn get_server_config(&self) -> ServerConfig {
        ServerConfig {
            host: "127.0.0.1".to_string(),
            port: self.server_port,
            max_connections: 100,
            heartbeat_interval_seconds: 5,
            client_timeout_seconds: 10,
            enable_compression: true,
            max_message_size: 4 * 1024 * 1024,
        }
    }

    fn get_database_config(&self) -> DatabaseConfig {
        DatabaseConfig {
            influx_url: format!("http://127.0.0.1:{}", self.influx_port),
            bucket: "test_market_data".to_string(),
            org: "test_org".to_string(),
            token: Some("test_token".to_string()),
            connection_pool_size: 5,
            connection_timeout_seconds: 5,
            write_timeout_seconds: 3,
            query_timeout_seconds: 10,
            retry_attempts: 2,
            retry_delay_ms: 100,
            circuit_breaker_threshold: 3,
            circuit_breaker_timeout_seconds: 30,
        }
    }
}

/// Test fixture for setting up the complete system
struct TestFixture {
    config: TestConfig,
    server_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
    client: MarketDataServiceClient<Channel>,
    hf_storage: Arc<HighFrequencyStorage>,
    citadel: Arc<Citadel>,
}

impl TestFixture {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = TestConfig::new();

        // Note: Using mock InfluxDB configuration for testing
        // In a real deployment, this would connect to an actual InfluxDB instance

        // Create system components
        let subscription_manager = Arc::new(SubscriptionManager::new());
        let influx_config = InfluxConfig {
            url: config.get_database_config().influx_url.clone(),
            bucket: config.get_database_config().bucket.clone(),
            org: config.get_database_config().org.clone(),
            token: config.get_database_config().token.clone(),
            pool_size: config.get_database_config().connection_pool_size,
            timeout: Duration::from_secs(config.get_database_config().connection_timeout_seconds),
            retry_attempts: config.get_database_config().retry_attempts,
            retry_delay: Duration::from_millis(config.get_database_config().retry_delay_ms),
            batch_size: 1000,
            flush_interval: Duration::from_millis(100),
        };
        let influx_client = Arc::new(InfluxClient::new(influx_config));
        let hf_storage = Arc::new(HighFrequencyStorage::new());
        let metrics =
            Arc::new(MetricsCollector::new().expect("Failed to create metrics collector"));

        // Create Citadel for data coordination
        let citadel_config = CitadelConfig::default();
        let citadel = Arc::new(Citadel::new(
            citadel_config,
            Arc::clone(&influx_client),
            Arc::clone(&subscription_manager),
        ));

        // Create and start server
        let server = MarketDataServer::with_metrics(
            subscription_manager,
            influx_client,
            Arc::clone(&hf_storage),
            metrics,
            config.get_server_config().max_connections,
        );

        let server_config = config.get_server_config();

        let server_handle =
            tokio::spawn(
                async move { server.start(&server_config.host, server_config.port).await },
            );

        // Wait for server to start
        sleep(Duration::from_millis(500)).await;

        // Create client
        let client_addr = format!("http://127.0.0.1:{}", config.server_port);
        let client = MarketDataServiceClient::connect(client_addr).await?;

        Ok(TestFixture {
            config,
            server_handle,
            client,
            hf_storage,
            citadel,
        })
    }

    async fn shutdown(self) {
        self.server_handle.abort();
        let _ = self.server_handle.await;
    }
}

/// Helper function to create test market data
fn create_test_orderbook_data(symbol: &str, sequence: u64) -> OrderBookData {
    let mut rng = rand::thread_rng();
    let base_price = 45000.0 + rng.gen_range(-1000.0..1000.0);

    OrderBookData {
        symbol: symbol.to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids: vec![
            (base_price - 1.0, rng.gen_range(0.1..5.0)),
            (base_price - 2.0, rng.gen_range(0.1..5.0)),
        ],
        asks: vec![
            (base_price + 1.0, rng.gen_range(0.1..5.0)),
            (base_price + 2.0, rng.gen_range(0.1..5.0)),
        ],
        sequence,
        exchange: "test_exchange".to_string(),
    }
}

fn create_test_trade_data(symbol: &str, trade_id: &str) -> TradeData {
    let mut rng = rand::thread_rng();
    let price = 45000.0 + rng.gen_range(-1000.0..1000.0);

    TradeData {
        symbol: symbol.to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        price,
        quantity: rng.gen_range(0.01..1.0),
        side: if rng.gen_bool(0.5) { "buy" } else { "sell" }.to_string(),
        trade_id: trade_id.to_string(),
        exchange: "test_exchange".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_startup_and_shutdown() {
        println!("🏰 Testing server startup and shutdown...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");

        // Test that server is running by making a simple request
        let mut client = fixture.client.clone();
        let request = Request::new(SubscribeRequest {
            client_id: "test_startup_client".to_string(),
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![DataType::Orderbook as i32],
            filters: HashMap::new(),
        });

        let response = client.subscribe(request).await;
        assert!(response.is_ok(), "Server should be responsive");

        let subscribe_response = response.unwrap().into_inner();
        assert!(subscribe_response.success, "Subscription should succeed");

        println!("✅ Server startup and basic connectivity test passed");

        fixture.shutdown().await;
        println!("✅ Server shutdown test passed");
    }

    #[tokio::test]
    async fn test_grpc_client_server_communication() {
        println!("🔗 Testing gRPC client-server communication...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");
        let mut client = fixture.client.clone();

        // Test Subscribe RPC
        let client_id = format!("test_client_{}", Uuid::new_v4());
        let subscribe_request = Request::new(SubscribeRequest {
            client_id: client_id.clone(),
            symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
            data_types: vec![DataType::Orderbook as i32, DataType::Trades as i32],
            filters: HashMap::new(),
        });

        let subscribe_response = client.subscribe(subscribe_request).await;
        assert!(
            subscribe_response.is_ok(),
            "Subscribe request should succeed"
        );

        let response = subscribe_response.unwrap().into_inner();
        assert!(response.success, "Subscription should be successful");
        assert!(
            !response.subscribed_topics.is_empty(),
            "Should have subscribed topics"
        );

        println!("✅ Subscribe RPC test passed");

        // Test Unsubscribe RPC
        let unsubscribe_request = Request::new(UnsubscribeRequest {
            client_id: client_id.clone(),
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![DataType::Orderbook as i32],
        });

        let unsubscribe_response = client.unsubscribe(unsubscribe_request).await;
        assert!(
            unsubscribe_response.is_ok(),
            "Unsubscribe request should succeed"
        );

        let response = unsubscribe_response.unwrap().into_inner();
        assert!(response.success, "Unsubscription should be successful");

        println!("✅ Unsubscribe RPC test passed");

        // Test Historical Data RPC
        let historical_request = Request::new(HistoricalDataRequest {
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![DataType::Orderbook as i32],
            start_time: chrono::Utc::now().timestamp_millis() - 3600000, // 1 hour ago
            end_time: chrono::Utc::now().timestamp_millis(),
            limit: 100,
        });

        let historical_response = client.get_historical_data(historical_request).await;
        assert!(
            historical_response.is_ok(),
            "Historical data request should succeed"
        );

        println!("✅ Historical data RPC test passed");

        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn test_bidirectional_streaming() {
        println!("🔄 Testing bidirectional streaming...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");
        let mut client = fixture.client.clone();

        // Create bidirectional stream
        let (sender, receiver) = mpsc::unbounded_channel();
        let request_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);

        let response = client
            .stream_market_data(Request::new(request_stream))
            .await;
        assert!(response.is_ok(), "Stream creation should succeed");

        let mut response_stream = response.unwrap().into_inner();

        // Send subscription request
        let client_id = format!("streaming_client_{}", Uuid::new_v4());
        let subscribe_msg = SubscriptionRequest {
            request: Some(
                market_data_subscription_server::proto::subscription_request::Request::Subscribe(
                    SubscribeRequest {
                        client_id: client_id.clone(),
                        symbols: vec!["BTCUSDT".to_string()],
                        data_types: vec![DataType::Orderbook as i32],
                        filters: HashMap::new(),
                    },
                ),
            ),
        };

        sender
            .send(subscribe_msg)
            .expect("Failed to send subscription");

        // Inject some test data
        let test_data = create_test_orderbook_data("BTCUSDT", 1);
        fixture.hf_storage.update_orderbook(&test_data);

        // Wait for and verify response
        let response_timeout = timeout(Duration::from_secs(5), response_stream.next()).await;
        assert!(
            response_timeout.is_ok(),
            "Should receive response within timeout"
        );

        let response = response_timeout.unwrap();
        assert!(response.is_some(), "Should receive a message");

        let message = response.unwrap();
        assert!(message.is_ok(), "Message should be valid");

        println!("✅ Bidirectional streaming test passed");

        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn test_complete_data_flow_end_to_end() {
        println!("🌊 Testing complete data flow from ingestion to client delivery...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");
        let mut client = fixture.client.clone();

        // Set up streaming client
        let (sender, receiver) = mpsc::unbounded_channel();
        let request_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);

        let response = client
            .stream_market_data(Request::new(request_stream))
            .await;
        assert!(response.is_ok(), "Stream creation should succeed");

        let mut response_stream = response.unwrap().into_inner();

        // Subscribe to data
        let client_id = format!("e2e_client_{}", Uuid::new_v4());
        let subscribe_msg = SubscriptionRequest {
            request: Some(
                market_data_subscription_server::proto::subscription_request::Request::Subscribe(
                    SubscribeRequest {
                        client_id: client_id.clone(),
                        symbols: vec!["BTCUSDT".to_string()],
                        data_types: vec![DataType::Orderbook as i32, DataType::Trades as i32],
                        filters: HashMap::new(),
                    },
                ),
            ),
        };

        sender
            .send(subscribe_msg)
            .expect("Failed to send subscription");

        // Simulate WebSocket data ingestion
        println!("📡 Simulating WebSocket data ingestion...");

        // Inject orderbook data
        for i in 1..=5 {
            let orderbook_data = create_test_orderbook_data("BTCUSDT", i);
            fixture.hf_storage.update_orderbook(&orderbook_data);
            sleep(Duration::from_millis(10)).await;
        }

        // Inject trade data
        for i in 1..=5 {
            let trade_data = create_test_trade_data("BTCUSDT", &format!("trade_{i}"));
            fixture.hf_storage.update_trade(&trade_data);
            sleep(Duration::from_millis(10)).await;
        }

        // Collect responses
        let mut received_messages = Vec::new();
        let collection_timeout = timeout(Duration::from_secs(10), async {
            while let Some(message_result) = response_stream.next().await {
                if let Ok(message) = message_result {
                    received_messages.push(message);
                    if received_messages.len() >= 5 {
                        break;
                    }
                }
            }
        })
        .await;

        assert!(
            collection_timeout.is_ok(),
            "Should receive messages within timeout"
        );
        assert!(
            !received_messages.is_empty(),
            "Should receive at least one message"
        );

        // Verify message content
        let has_orderbook = received_messages.iter().any(|msg| {
            matches!(
                msg.data,
                Some(
                    market_data_subscription_server::proto::market_data_message::Data::Orderbook(_)
                )
            )
        });

        assert!(has_orderbook, "Should receive orderbook data");

        println!(
            "✅ End-to-end data flow test passed - received {} messages",
            received_messages.len()
        );

        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn test_influxdb_integration() {
        println!("🏦 Testing InfluxDB integration...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");

        // Test database connectivity
        let influx_config = InfluxConfig {
            url: fixture.config.get_database_config().influx_url,
            bucket: fixture.config.get_database_config().bucket,
            org: fixture.config.get_database_config().org,
            token: fixture.config.get_database_config().token,
            pool_size: 5,
            timeout: Duration::from_secs(5),
            retry_attempts: 2,
            retry_delay: Duration::from_millis(100),
            batch_size: 100,
            flush_interval: Duration::from_millis(50),
        };

        let influx_client = InfluxClient::new(influx_config);

        // Test basic connectivity (ping)
        let ping_result = influx_client.ping().await;
        assert!(ping_result.is_ok(), "InfluxDB should be reachable");

        println!("✅ InfluxDB connectivity test passed");

        // Test data writing through Citadel
        let test_data = create_test_orderbook_data("BTCUSDT", 1);
        fixture.hf_storage.update_orderbook(&test_data);

        // Test data writing through Citadel directly
        // Note: In the current implementation, Citadel handles data processing
        // The snapshot service would be a separate component in a full deployment
        let test_orderbook = create_test_orderbook_data("BTCUSDT", 1);
        let process_result = fixture
            .citadel
            .process_orderbook_data("BTCUSDT", test_orderbook)
            .await;
        assert!(
            process_result.is_ok(),
            "Citadel data processing should succeed"
        );

        println!("✅ InfluxDB data writing test passed");

        // Test historical data query
        let start_time = chrono::Utc::now().timestamp_millis() - 3600000; // 1 hour ago
        let end_time = chrono::Utc::now().timestamp_millis();

        let query_result = influx_client
            .query_historical_data("orderbook", "BTCUSDT", start_time, end_time, Some(100))
            .await;

        // Note: Query might return empty results in test environment, but should not error
        assert!(
            query_result.is_ok(),
            "Historical data query should not error"
        );

        println!("✅ InfluxDB historical data query test passed");

        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn test_concurrent_client_connections() {
        println!("👥 Testing concurrent client connections...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");

        // Create multiple concurrent clients
        let num_clients = 10;
        let mut client_handles = Vec::new();

        for i in 0..num_clients {
            let client_addr = format!("http://127.0.0.1:{}", fixture.config.server_port);
            let client_id = format!("concurrent_client_{i}");

            let handle = tokio::spawn(async move {
                let mut client = MarketDataServiceClient::connect(client_addr)
                    .await
                    .expect("Failed to connect client");

                // Subscribe to data
                let subscribe_request = Request::new(SubscribeRequest {
                    client_id: client_id.clone(),
                    symbols: vec!["BTCUSDT".to_string()],
                    data_types: vec![DataType::Orderbook as i32],
                    filters: HashMap::new(),
                });

                let response = client.subscribe(subscribe_request).await;
                assert!(response.is_ok(), "Client {i} subscription should succeed");

                let subscribe_response = response.unwrap().into_inner();
                assert!(
                    subscribe_response.success,
                    "Client {i} subscription should be successful"
                );

                // Unsubscribe
                let unsubscribe_request = Request::new(UnsubscribeRequest {
                    client_id: client_id.clone(),
                    symbols: vec!["BTCUSDT".to_string()],
                    data_types: vec![DataType::Orderbook as i32],
                });

                let response = client.unsubscribe(unsubscribe_request).await;
                assert!(response.is_ok(), "Client {i} unsubscription should succeed");

                i
            });

            client_handles.push(handle);
        }

        // Wait for all clients to complete
        let results = futures_util::future::join_all(client_handles).await;

        // Verify all clients succeeded
        for (i, result) in results.into_iter().enumerate() {
            assert!(
                result.is_ok(),
                "Client {i} task should complete successfully"
            );
            let client_id = result.unwrap();
            assert_eq!(client_id, i, "Client ID should match");
        }

        println!("✅ Concurrent client connections test passed - {num_clients} clients");

        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn test_load_scenario_high_frequency_data() {
        println!("⚡ Testing load scenario with high-frequency data...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");
        let mut client = fixture.client.clone();

        // Set up streaming client
        let (sender, receiver) = mpsc::unbounded_channel();
        let request_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);

        let response = client
            .stream_market_data(Request::new(request_stream))
            .await;
        assert!(response.is_ok(), "Stream creation should succeed");

        let mut response_stream = response.unwrap().into_inner();

        // Subscribe to multiple symbols
        let client_id = format!("load_test_client_{}", Uuid::new_v4());
        let symbols = vec!["BTCUSDT", "ETHUSDT", "ADAUSDT", "DOTUSDT", "LINKUSDT"];

        let subscribe_msg = SubscriptionRequest {
            request: Some(
                market_data_subscription_server::proto::subscription_request::Request::Subscribe(
                    SubscribeRequest {
                        client_id: client_id.clone(),
                        symbols: symbols.iter().map(|s| s.to_string()).collect(),
                        data_types: vec![DataType::Orderbook as i32, DataType::Trades as i32],
                        filters: HashMap::new(),
                    },
                ),
            ),
        };

        sender
            .send(subscribe_msg)
            .expect("Failed to send subscription");

        // Generate high-frequency data
        println!("📊 Generating high-frequency market data...");
        let data_generation_handle = {
            let hf_storage = Arc::clone(&fixture.hf_storage);
            tokio::spawn(async move {
                for sequence in 1..=1000 {
                    for symbol in &symbols {
                        // Generate orderbook update
                        let orderbook_data = create_test_orderbook_data(symbol, sequence);
                        hf_storage.update_orderbook(&orderbook_data);

                        // Generate trade update
                        let trade_data =
                            create_test_trade_data(symbol, &format!("{symbol}_{sequence}"));
                        hf_storage.update_trade(&trade_data);
                    }

                    // Small delay to simulate realistic update frequency
                    if sequence % 100 == 0 {
                        sleep(Duration::from_millis(1)).await;
                    }
                }
            })
        };

        // Collect responses for a limited time
        let mut received_count = 0;
        let collection_timeout = timeout(Duration::from_secs(15), async {
            while let Some(message_result) = response_stream.next().await {
                if message_result.is_ok() {
                    received_count += 1;
                    if received_count >= 100 {
                        break;
                    }
                }
            }
        })
        .await;

        // Wait for data generation to complete
        let _ = data_generation_handle.await;

        assert!(
            collection_timeout.is_ok(),
            "Should handle high-frequency data within timeout"
        );
        assert!(received_count > 0, "Should receive messages under load");

        println!("✅ Load scenario test passed - processed {received_count} messages");

        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn test_error_handling_and_recovery() {
        println!("🛡️ Testing error handling and recovery...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");
        let mut client = fixture.client.clone();

        // Test invalid subscription request
        let invalid_request = Request::new(SubscribeRequest {
            client_id: "".to_string(), // Empty client ID
            symbols: vec![],           // Empty symbols
            data_types: vec![],        // Empty data types
            filters: HashMap::new(),
        });

        let response = client.subscribe(invalid_request).await;
        assert!(
            response.is_ok(),
            "Server should handle invalid requests gracefully"
        );

        let _subscribe_response = response.unwrap().into_inner();
        // The server might still succeed with empty subscriptions, but should handle it gracefully

        println!("✅ Invalid request handling test passed");

        // Test client disconnection handling
        let client_id = format!("disconnect_test_client_{}", Uuid::new_v4());
        let (sender, receiver) = mpsc::unbounded_channel();
        let request_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);

        let response = client
            .stream_market_data(Request::new(request_stream))
            .await;
        assert!(response.is_ok(), "Stream creation should succeed");

        let mut response_stream = response.unwrap().into_inner();

        // Subscribe
        let subscribe_msg = SubscriptionRequest {
            request: Some(
                market_data_subscription_server::proto::subscription_request::Request::Subscribe(
                    SubscribeRequest {
                        client_id: client_id.clone(),
                        symbols: vec!["BTCUSDT".to_string()],
                        data_types: vec![DataType::Orderbook as i32],
                        filters: HashMap::new(),
                    },
                ),
            ),
        };

        sender
            .send(subscribe_msg)
            .expect("Failed to send subscription");

        // Simulate client disconnection by dropping the sender
        drop(sender);

        // The stream should eventually end
        let mut stream_ended = false;
        let disconnect_timeout = timeout(Duration::from_secs(5), async {
            while (response_stream.next().await).is_some() {
                // Continue until stream ends
            }
            stream_ended = true;
        })
        .await;

        // Either timeout or stream ended gracefully
        assert!(
            disconnect_timeout.is_ok() || stream_ended,
            "Server should handle client disconnection"
        );

        println!("✅ Client disconnection handling test passed");

        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn test_subscription_management() {
        println!("📋 Testing subscription management...");

        let fixture = TestFixture::new()
            .await
            .expect("Failed to create test fixture");
        let mut client = fixture.client.clone();

        let client_id = format!("subscription_mgmt_client_{}", Uuid::new_v4());

        // Test multiple subscriptions
        let symbols = vec!["BTCUSDT", "ETHUSDT", "ADAUSDT"];
        let data_types = vec![DataType::Orderbook, DataType::Trades];

        for symbol in &symbols {
            for data_type in &data_types {
                let subscribe_request = Request::new(SubscribeRequest {
                    client_id: client_id.clone(),
                    symbols: vec![symbol.to_string()],
                    data_types: vec![*data_type as i32],
                    filters: HashMap::new(),
                });

                let response = client.subscribe(subscribe_request).await;
                assert!(
                    response.is_ok(),
                    "Subscription to {symbol}::{data_type:?} should succeed"
                );

                let subscribe_response = response.unwrap().into_inner();
                assert!(
                    subscribe_response.success,
                    "Subscription should be successful"
                );
            }
        }

        println!("✅ Multiple subscriptions test passed");

        // Test partial unsubscription
        let unsubscribe_request = Request::new(UnsubscribeRequest {
            client_id: client_id.clone(),
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![DataType::Orderbook as i32],
        });

        let response = client.unsubscribe(unsubscribe_request).await;
        assert!(response.is_ok(), "Partial unsubscription should succeed");

        let unsubscribe_response = response.unwrap().into_inner();
        assert!(
            unsubscribe_response.success,
            "Unsubscription should be successful"
        );

        println!("✅ Partial unsubscription test passed");

        // Test complete unsubscription
        for symbol in &symbols {
            for data_type in &data_types {
                let unsubscribe_request = Request::new(UnsubscribeRequest {
                    client_id: client_id.clone(),
                    symbols: vec![symbol.to_string()],
                    data_types: vec![*data_type as i32],
                });

                let response = client.unsubscribe(unsubscribe_request).await;
                assert!(
                    response.is_ok(),
                    "Unsubscription from {symbol}::{data_type:?} should succeed"
                );
            }
        }

        println!("✅ Complete unsubscription test passed");

        fixture.shutdown().await;
    }
}

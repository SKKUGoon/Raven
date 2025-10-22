// gRPC service implementation for MarketDataService

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error, info, warn};

use crate::citadel::storage::HighFrequencyStorage;
use crate::database::influx_client::InfluxClient;
use crate::exchanges::types::parse_exchange_symbol_key;
use crate::monitoring::MetricsCollector;
use crate::proto::{
    market_data_service_server::MarketDataService, DataType, HistoricalDataRequest,
    MarketDataMessage, SubscribeRequest, SubscribeResponse, SubscriptionRequest,
    UnsubscribeRequest, UnsubscribeResponse,
};
use crate::subscription_manager::{SubscriptionDataType, SubscriptionManager};

use super::connection::ConnectionManager;
use crate::error::RavenResult;

/// gRPC service implementation
#[derive(Clone)]
pub struct MarketDataServiceImpl {
    /// The Maester's Registry - manages all client subscriptions
    subscription_manager: Arc<SubscriptionManager>,
    /// The Iron Bank - InfluxDB client for historical data
    influx_client: Arc<InfluxClient>,
    /// High-frequency atomic storage for real-time data
    hf_storage: Arc<HighFrequencyStorage>,
    /// Metrics collector for monitoring
    metrics: Option<Arc<MetricsCollector>>,
    /// Connection manager
    connection_manager: ConnectionManager,
}

impl MarketDataServiceImpl {
    /// Create a new MarketDataServiceImpl
    pub fn new(
        subscription_manager: Arc<SubscriptionManager>,
        influx_client: Arc<InfluxClient>,
        hf_storage: Arc<HighFrequencyStorage>,
        metrics: Option<Arc<MetricsCollector>>,
        connection_manager: ConnectionManager,
    ) -> Self {
        Self {
            subscription_manager,
            influx_client,
            hf_storage,
            metrics,
            connection_manager,
        }
    }

    /// Convert DataType to SubscriptionDataType
    pub fn convert_data_type(data_type: DataType) -> SubscriptionDataType {
        match data_type {
            DataType::Orderbook => SubscriptionDataType::Orderbook,
            DataType::Trades => SubscriptionDataType::Trades,
            DataType::Candles1m => SubscriptionDataType::Candles1M,
            DataType::Candles5m => SubscriptionDataType::Candles5M,
            DataType::Candles1h => SubscriptionDataType::Candles1H,
            DataType::FundingRates => SubscriptionDataType::FundingRates,
            DataType::WalletUpdates => SubscriptionDataType::WalletUpdates,
        }
    }

    /// Create market data message from orderbook snapshot
    pub fn create_orderbook_message(
        symbol: &str,
        best_bid_price: f64,
        best_bid_quantity: f64,
        best_ask_price: f64,
        best_ask_quantity: f64,
        sequence: u64,
        timestamp: i64,
    ) -> MarketDataMessage {
        use crate::proto::{OrderBookSnapshot, PriceLevel};

        let orderbook = OrderBookSnapshot {
            symbol: symbol.to_string(),
            timestamp,
            bids: vec![PriceLevel {
                price: best_bid_price,
                quantity: best_bid_quantity,
            }],
            asks: vec![PriceLevel {
                price: best_ask_price,
                quantity: best_ask_quantity,
            }],
            sequence: sequence as i64,
        };

        MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Orderbook(
                orderbook,
            )),
        }
    }

    /// Create market data message from trade snapshot
    pub fn create_trade_message(
        symbol: &str,
        price: f64,
        quantity: f64,
        side: &str,
        trade_id: u64,
        timestamp: i64,
    ) -> MarketDataMessage {
        use crate::proto::Trade;

        let trade = Trade {
            symbol: symbol.to_string(),
            timestamp,
            price,
            quantity,
            side: side.to_string(),
            trade_id: trade_id.to_string(),
        };

        MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Trade(trade)),
        }
    }

    /// Stream real-time data from atomic storage to client
    async fn stream_realtime_data(
        &self,
        client_id: String,
        mut receiver: mpsc::UnboundedReceiver<MarketDataMessage>,
    ) -> RavenResult<Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>> {
        info!("⟐ Starting real-time data stream for client: {}", client_id);
        let hf_storage = Arc::clone(&self.hf_storage);

        // Create a stream that combines real-time atomic reads with subscription messages
        let stream = async_stream::stream! {
            // Send initial snapshots for all symbols
            for key in hf_storage.get_orderbook_symbols() {
                if let Some((exchange, symbol)) = parse_exchange_symbol_key(&key) {
                    if let Some(snapshot) = hf_storage.get_orderbook_snapshot(&symbol, &exchange) {
                    let message = Self::create_orderbook_message(
                        &snapshot.symbol,
                        snapshot.best_bid_price,
                        snapshot.best_bid_quantity,
                        snapshot.best_ask_price,
                        snapshot.best_ask_quantity,
                        snapshot.sequence,
                        snapshot.timestamp,
                    );
                    yield Ok(message);
                    }
                }
            }

            for key in hf_storage.get_trade_symbols() {
                if let Some((exchange, symbol)) = parse_exchange_symbol_key(&key) {
                    if let Some(snapshot) = hf_storage.get_trade_snapshot(&symbol, &exchange) {
                    let message = Self::create_trade_message(
                        &snapshot.symbol,
                        snapshot.price,
                        snapshot.quantity,
                        snapshot.side.as_str(),
                        snapshot.trade_id,
                        snapshot.timestamp,
                    );
                    yield Ok(message);
                    }
                }
            }

            // Stream subscription messages
            while let Some(message) = receiver.recv().await {
                yield Ok(message);
            }
        };

        Ok(Box::pin(stream))
    }

    /// Query historical data from InfluxDB
    async fn query_historical_data_internal(
        &self,
        symbols: Vec<String>,
        data_types: Vec<DataType>,
        start_time: i64,
        end_time: i64,
        limit: Option<u32>,
    ) -> RavenResult<Vec<MarketDataMessage>> {
        let messages = Vec::new();

        for symbol in symbols {
            for data_type in &data_types {
                let measurement = match data_type {
                    DataType::Orderbook => "orderbook",
                    DataType::Trades => "trades",
                    DataType::Candles1m | DataType::Candles5m | DataType::Candles1h => "candles",
                    DataType::FundingRates => "funding_rates",
                    DataType::WalletUpdates => "wallet_updates",
                };

                match self
                    .influx_client
                    .query_historical_data(measurement, &symbol, start_time, end_time, limit)
                    .await
                {
                    Ok(results) => {
                        if !results.is_empty() {
                            debug!(
                                "Retrieved {} historical records for {}::{:?}",
                                results.len(),
                                symbol,
                                data_type
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            "⚠ Failed to query historical data for {}::{:?}: {}",
                            symbol, data_type, e
                        );
                    }
                }
            }
        }

        Ok(messages)
    }
}

#[tonic::async_trait]
impl MarketDataService for MarketDataServiceImpl {
    type StreamMarketDataStream =
        Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;
    type GetHistoricalDataStream =
        Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    /// Bidirectional streaming service for real-time market data
    async fn stream_market_data(
        &self,
        request: Request<Streaming<SubscriptionRequest>>,
    ) -> Result<Response<Self::StreamMarketDataStream>, Status> {
        info!("⟐ New gRPC streaming connection attempt");

        // Check connection limits
        if !self.connection_manager.can_accept_connection().await {
            error!("⊘ Connection rejected - maximum connections reached");
            return Err(Status::resource_exhausted("Maximum connections reached"));
        }

        self.connection_manager
            .increment_connections(self.metrics.as_ref())
            .await
            .map_err(|e| {
                error!("✗ Failed to accept connection: {}", e);
                Status::internal(format!("Failed to accept connection: {e}"))
            })?;

        let connection_start = std::time::Instant::now();
        info!("✓ gRPC streaming connection established successfully");

        let mut stream = request.into_inner();
        let subscription_manager = Arc::clone(&self.subscription_manager);
        let server = self.clone();

        // Create channel for sending messages to client
        let (sender, receiver) = mpsc::unbounded_channel();
        let client_id = Arc::new(RwLock::new(String::new()));

        // Handle incoming subscription requests
        let subscription_handler = {
            let subscription_manager = Arc::clone(&subscription_manager);
            let sender = sender.clone();
            let client_id_clone = Arc::clone(&client_id);

            async move {
                while let Some(request_result) = stream.next().await {
                    match request_result {
                        Ok(subscription_request) => {
                            info!("⟐ Received gRPC subscription request");
                            match subscription_request.request {
                                Some(crate::proto::subscription_request::Request::Subscribe(
                                    sub_req,
                                )) => {
                                    info!(
                                        "⚬ Processing Subscribe request for client: {}",
                                        sub_req.client_id
                                    );
                                    *client_id_clone.write().await = sub_req.client_id.clone();

                                    let symbols = sub_req.symbols;
                                    let data_types: Vec<SubscriptionDataType> = sub_req
                                        .data_types
                                        .into_iter()
                                        .map(|dt| {
                                            let data_type = DataType::try_from(dt)
                                                .unwrap_or(DataType::Orderbook);
                                            Self::convert_data_type(data_type)
                                        })
                                        .collect();

                                    let filters: HashMap<String, String> = sub_req.filters;

                                    match subscription_manager.subscribe(
                                        sub_req.client_id.clone(),
                                        symbols.clone(),
                                        data_types.clone(),
                                        filters,
                                        sender.clone(),
                                    ) {
                                        Ok(topics) => {
                                            info!(
                                                "✓ Client {} subscribed to {} topics: {:?}",
                                                sub_req.client_id,
                                                topics.len(),
                                                symbols
                                            );
                                        }
                                        Err(e) => {
                                            error!(
                                                "✗ Subscription failed for client {}: {}",
                                                sub_req.client_id, e
                                            );
                                        }
                                    }
                                }
                                Some(crate::proto::subscription_request::Request::Unsubscribe(
                                    unsub_req,
                                )) => {
                                    let symbols = unsub_req.symbols;
                                    let data_types: Vec<SubscriptionDataType> = unsub_req
                                        .data_types
                                        .into_iter()
                                        .map(|dt| {
                                            let data_type = DataType::try_from(dt)
                                                .unwrap_or(DataType::Orderbook);
                                            Self::convert_data_type(data_type)
                                        })
                                        .collect();

                                    match subscription_manager.unsubscribe(
                                        &unsub_req.client_id,
                                        symbols,
                                        data_types,
                                    ) {
                                        Ok(unsubscribed_topics) => {
                                            info!(
                                                "✓ Client {} unsubscribed from {} topics",
                                                unsub_req.client_id,
                                                unsubscribed_topics.len()
                                            );
                                        }
                                        Err(e) => {
                                            error!(
                                                "✗ Unsubscription failed for client {}: {}",
                                                unsub_req.client_id, e
                                            );
                                        }
                                    }
                                }
                                Some(crate::proto::subscription_request::Request::Heartbeat(
                                    heartbeat_req,
                                )) => {
                                    info!(
                                        "♡ Received heartbeat from client: {}",
                                        heartbeat_req.client_id
                                    );
                                    if let Err(e) = subscription_manager
                                        .update_heartbeat(&heartbeat_req.client_id)
                                    {
                                        error!(
                                            "✗ Heartbeat update failed for client {}: {}",
                                            heartbeat_req.client_id, e
                                        );
                                    }
                                }
                                None => {
                                    error!("✗ Received empty subscription request");
                                }
                            }
                        }
                        Err(e) => {
                            error!("✗ Error receiving subscription request: {}", e);
                            break;
                        }
                    }
                }

                // Clean up when stream ends
                let final_client_id = client_id_clone.read().await.clone();
                if !final_client_id.is_empty() {
                    if let Err(e) = subscription_manager.unsubscribe_all(&final_client_id) {
                        error!("✗ Failed to clean up client {}: {}", final_client_id, e);
                    }
                    info!("⚬ Cleaned up client {} subscription", final_client_id);
                }
            }
        };

        // Spawn the subscription handler
        tokio::spawn(subscription_handler);

        // Create the response stream with a placeholder client ID
        // The actual client ID will be set when the first subscription request arrives
        info!("▶ Creating response stream for new client");
        let response_stream = server
            .stream_realtime_data("pending".to_string(), receiver)
            .await
            .map_err(|e| {
                error!("✗ Failed to create response stream: {}", e);
                Status::internal(format!("Failed to create stream: {e}"))
            })?;

        // Decrement connection count when stream ends
        let server_clone = server.clone();

        // Wrap the stream to handle cleanup
        let cleanup_stream = async_stream::stream! {
            tokio::pin!(response_stream);
            while let Some(item) = response_stream.next().await {
                yield item;
            }
            // Cleanup when stream ends
            let connection_duration = connection_start.elapsed();
            server_clone.connection_manager.decrement_connections(connection_duration, server_clone.metrics.as_ref()).await;
            info!("⟐ gRPC streaming connection closed (duration: {:?})", connection_duration);
        };

        Ok(Response::new(Box::pin(cleanup_stream)))
    }

    /// Subscribe to specific market data streams
    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<SubscribeResponse>, Status> {
        let req = request.into_inner();
        let client_id = req.client_id.clone();

        info!(
            "⚬ gRPC Subscribe request from client: {} for symbols: {:?}",
            client_id, req.symbols
        );

        let symbols = req.symbols;
        let data_types: Vec<SubscriptionDataType> = req
            .data_types
            .into_iter()
            .map(|dt| {
                let data_type = DataType::try_from(dt).unwrap_or(DataType::Orderbook);
                Self::convert_data_type(data_type)
            })
            .collect();
        let filters: HashMap<String, String> = req.filters;

        // Create a dummy sender for non-streaming subscriptions
        let (sender, _receiver) = mpsc::unbounded_channel();

        match self.subscription_manager.subscribe(
            client_id.clone(),
            symbols.clone(),
            data_types.clone(),
            filters,
            sender,
        ) {
            Ok(topics) => {
                info!(
                    "✓ Client {} subscribed to {} topics: {:?}",
                    client_id,
                    topics.len(),
                    symbols
                );

                let response = SubscribeResponse {
                    success: true,
                    message: format!("Successfully subscribed to {} topics", topics.len()),
                    subscribed_topics: topics,
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                error!("✗ Subscription failed for client {}: {}", client_id, e);

                let response = SubscribeResponse {
                    success: false,
                    message: format!("Subscription failed: {e}"),
                    subscribed_topics: vec![],
                };

                Ok(Response::new(response))
            }
        }
    }

    /// Unsubscribe from specific market data streams
    async fn unsubscribe(
        &self,
        request: Request<UnsubscribeRequest>,
    ) -> Result<Response<UnsubscribeResponse>, Status> {
        let req = request.into_inner();
        let client_id = req.client_id.clone();

        info!("⚬ Unsubscribe request from client: {}", client_id);

        let symbols = req.symbols;
        let data_types: Vec<SubscriptionDataType> = req
            .data_types
            .into_iter()
            .map(|dt| {
                let data_type = DataType::try_from(dt).unwrap_or(DataType::Orderbook);
                Self::convert_data_type(data_type)
            })
            .collect();

        match self
            .subscription_manager
            .unsubscribe(&client_id, symbols, data_types)
        {
            Ok(unsubscribed_topics) => {
                info!(
                    "✓ Client {} unsubscribed from {} topics",
                    client_id,
                    unsubscribed_topics.len()
                );

                let response = UnsubscribeResponse {
                    success: true,
                    message: format!(
                        "Successfully unsubscribed from {} topics",
                        unsubscribed_topics.len()
                    ),
                    unsubscribed_topics,
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                error!("✗ Unsubscription failed for client {}: {}", client_id, e);

                let response = UnsubscribeResponse {
                    success: false,
                    message: format!("Unsubscription failed: {e}"),
                    unsubscribed_topics: vec![],
                };

                Ok(Response::new(response))
            }
        }
    }

    /// Get historical market data
    async fn get_historical_data(
        &self,
        request: Request<HistoricalDataRequest>,
    ) -> Result<Response<Self::GetHistoricalDataStream>, Status> {
        let req = request.into_inner();

        info!(
            "Historical data request for symbols: {:?}, types: {:?}",
            req.symbols, req.data_types
        );

        let symbols = req.symbols;
        let data_types: Vec<DataType> = req
            .data_types
            .into_iter()
            .map(|dt| DataType::try_from(dt).unwrap_or(DataType::Orderbook))
            .collect();
        let start_time = req.start_time;
        let end_time = req.end_time;
        let limit = if req.limit > 0 {
            Some(req.limit as u32)
        } else {
            None
        };

        // Validate time range
        if start_time >= end_time {
            return Err(Status::invalid_argument(
                "Start time must be before end time",
            ));
        }

        let server = self.clone();

        // Create streaming response for historical data
        let stream = async_stream::stream! {
            match server.query_historical_data_internal(
                symbols.clone(),
                data_types.clone(),
                start_time,
                end_time,
                limit,
            ).await {
                Ok(messages) => {
                    info!("Returning {} historical messages", messages.len());
                    for message in messages {
                        yield Ok(message);
                    }
                }
                Err(e) => {
                    error!("✗ Historical data query failed: {}", e);
                    yield Err(Status::internal(format!("Query failed: {e}")));
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }
}

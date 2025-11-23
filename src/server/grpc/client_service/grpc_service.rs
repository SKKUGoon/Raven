// Market Data Service Implementation
// "The Scribe's service for delivering the realm's data"

use crate::common::db::influx_client::InfluxClient;
use crate::proto::market_data_service_server::MarketDataService;
use crate::proto::{
    DataType, HistoricalDataRequest, MarketDataMessage, OrderBookSnapshot, PriceLevel,
    SubscribeRequest, SubscribeResponse, SubscriptionRequest, Trade, UnsubscribeRequest,
    UnsubscribeResponse,
};
use crate::server::data_engine::storage::HighFrequencyStorage;
use crate::server::grpc::client_service::manager::{ClientManager, DisconnectionReason};
use crate::server::monitoring::MetricsCollector;
use crate::server::stream_router::{StreamRouter, SubscriptionDataType};
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

/// Implementation of the MarketDataService gRPC interface
pub struct MarketDataServiceImpl {
    stream_router: Arc<StreamRouter>,
    _influx_client: Arc<InfluxClient>,
    _hf_storage: Arc<HighFrequencyStorage>,
    metrics: Option<Arc<MetricsCollector>>,
    client_manager: Arc<ClientManager>,
}

impl MarketDataServiceImpl {
    /// Create a new MarketDataServiceImpl
    pub fn new(
        stream_router: Arc<StreamRouter>,
        influx_client: Arc<InfluxClient>,
        hf_storage: Arc<HighFrequencyStorage>,
        metrics: Option<Arc<MetricsCollector>>,
        client_manager: Arc<ClientManager>,
    ) -> Self {
        Self {
            stream_router,
            _influx_client: influx_client,
            _hf_storage: hf_storage,
            metrics,
            client_manager,
        }
    }

    /// Helper to convert proto DataType to internal SubscriptionDataType
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

    /// Helper to create an orderbook message for testing
    pub fn create_orderbook_message(
        symbol: &str,
        bid_price: f64,
        bid_qty: f64,
        ask_price: f64,
        ask_qty: f64,
        sequence: i64,
        timestamp: i64,
    ) -> MarketDataMessage {
        MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Orderbook(
                OrderBookSnapshot {
                    symbol: symbol.to_string(),
                    timestamp,
                    bids: vec![PriceLevel {
                        price: bid_price,
                        quantity: bid_qty,
                    }],
                    asks: vec![PriceLevel {
                        price: ask_price,
                        quantity: ask_qty,
                    }],
                    sequence,
                },
            )),
        }
    }

    /// Helper to create a trade message for testing
    pub fn create_trade_message(
        symbol: &str,
        price: f64,
        quantity: f64,
        side: &str,
        trade_id: i64,
        timestamp: i64,
    ) -> MarketDataMessage {
        MarketDataMessage {
            data: Some(crate::proto::market_data_message::Data::Trade(Trade {
                symbol: symbol.to_string(),
                timestamp,
                price,
                quantity,
                side: side.to_string(),
                trade_id: trade_id.to_string(),
            })),
        }
    }
}

#[tonic::async_trait]
impl MarketDataService for MarketDataServiceImpl {
    type StreamMarketDataStream =
        Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    /// Stream market data based on subscription requests
    async fn stream_market_data(
        &self,
        request: Request<tonic::Streaming<SubscriptionRequest>>,
    ) -> Result<Response<Self::StreamMarketDataStream>, Status> {
        let peer_addr = request
            .remote_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        info!("New client connected: {}", peer_addr);

        // Check connection limits using ClientManager
        let can_accept =
            self.client_manager.get_client_count().await < self.client_manager.get_max_clients();

        if !can_accept {
            warn!("Connection limit reached, rejecting client: {}", peer_addr);

            // Record metric if available
            if let Some(metrics) = &self.metrics {
                metrics.record_error("max_connections_reached", "server");
            }

            return Err(Status::resource_exhausted(
                "Maximum connection limit reached",
            ));
        }

        let mut in_stream = request.into_inner();
        let (_tx, rx) = mpsc::channel(128);

        let _stream_router = self.stream_router.clone();
        let client_manager = self.client_manager.clone();
        let metrics = self.metrics.clone();

        // Spawn a task to handle incoming requests from this client
        tokio::spawn(async move {
            let client_id = uuid::Uuid::new_v4().to_string();

            // Register with ClientManager
            if let Err(e) = client_manager.register_client(client_id.clone()).await {
                error!("Failed to register client {}: {}", client_id, e);
                return; // Should reject connection if registration fails
            } else {
                // Add metadata
                let _ = client_manager
                    .add_client_metadata(&client_id, "peer_addr", &peer_addr)
                    .await;

                // Update metrics
                if let Some(m) = &metrics {
                    m.record_connection("grpc");
                    m.active_connections
                        .set(client_manager.get_client_count().await as f64);
                }
            }

            info!("Client session started: {} ({})", client_id, peer_addr);

            // Handle incoming messages
            while let Some(result) = in_stream.next().await {
                // Update activity/heartbeat in ClientManager
                let _ = client_manager.update_activity(&client_id).await;

                match result {
                    Ok(req) => {
                        let _ = client_manager.increment_messages_received(&client_id).await;

                        match req.request {
                            Some(crate::proto::subscription_request::Request::Subscribe(sub)) => {
                                info!("Client {} subscribing to {:?}", client_id, sub.symbols);
                                // TODO: Implement subscription logic properly
                                // stream_router.subscribe(&client_id, &sub.symbol).await;

                                // Dummy response for now since we need to return MarketDataMessage
                                // In a real implementation, this would come from the stream router
                            }
                            Some(crate::proto::subscription_request::Request::Unsubscribe(
                                unsub,
                            )) => {
                                info!(
                                    "Client {} unsubscribing from {:?}",
                                    client_id, unsub.symbols
                                );
                                // TODO: Implement unsubscribe logic properly
                                // stream_router.unsubscribe(&client_id, &unsub.symbol).await;
                            }
                            Some(crate::proto::subscription_request::Request::Heartbeat(_)) => {
                                // Heartbeat logic
                                let _ = client_manager.update_heartbeat(&client_id).await;
                            }
                            None => {
                                warn!("Received empty request from client {}", client_id);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving from client {}: {}", client_id, e);
                        break;
                    }
                }
            }

            info!("Client session ended: {}", client_id);

            // Unregister from ClientManager
            // This will also handle the cleanup and state update
            let _ = client_manager
                .disconnect_client(&client_id, DisconnectionReason::ClientInitiated)
                .await;

            // Force finalize immediately for this manual disconnection flow since we are exiting the task
            // Note: In a real scenario, we might want to let the grace period handle it, but since the stream ended, we know they are gone.
            // However, disconnect_client spawns a task to finalize after grace period, which is fine.

            // Update metrics
            if let Some(m) = &metrics {
                m.record_disconnection("grpc", std::time::Duration::from_secs(0)); // Duration is tracked in client manager events
                m.active_connections
                    .set(client_manager.get_client_count().await as f64);
            }
        });

        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(out_stream) as Self::StreamMarketDataStream
        ))
    }

    async fn subscribe(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> Result<Response<SubscribeResponse>, Status> {
        // For now, return unimplemented or a dummy response
        Ok(Response::new(SubscribeResponse {
            success: true,
            message: "Subscription successful".to_string(),
            subscribed_topics: vec![],
        }))
    }

    async fn unsubscribe(
        &self,
        _request: Request<UnsubscribeRequest>,
    ) -> Result<Response<UnsubscribeResponse>, Status> {
        Ok(Response::new(UnsubscribeResponse {
            success: true,
            message: "Unsubscription successful".to_string(),
            unsubscribed_topics: vec![],
        }))
    }

    type GetHistoricalDataStream =
        Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn get_historical_data(
        &self,
        _request: Request<HistoricalDataRequest>,
    ) -> Result<Response<Self::GetHistoricalDataStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        // Just close channel immediately for now
        drop(tx);

        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(out_stream) as Self::GetHistoricalDataStream
        ))
    }
}

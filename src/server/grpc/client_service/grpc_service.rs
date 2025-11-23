// Market Data Service Implementation
// "The Scribe's service for delivering the realm's data"

use crate::proto::market_data_service_server::MarketDataService;
use crate::proto::{
    DataType, HistoricalDataRequest, MarketDataMessage, OrderBookSnapshot, PriceLevel,
    SubscribeRequest, SubscribeResponse, SubscriptionRequest, Trade, UnsubscribeRequest,
    UnsubscribeResponse,
};
use crate::server::data_engine::storage::HighFrequencyStorage;
use crate::server::database::influx_client::InfluxClient;
use crate::server::grpc::client_service::connection::ConnectionManager;
use crate::server::monitoring::MetricsCollector;
use crate::server::subscription_manager::{SubscriptionDataType, SubscriptionManager};
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

/// Implementation of the MarketDataService gRPC interface
pub struct MarketDataServiceImpl {
    subscription_manager: Arc<SubscriptionManager>,
    _influx_client: Arc<InfluxClient>,
    _hf_storage: Arc<HighFrequencyStorage>,
    metrics: Option<Arc<MetricsCollector>>,
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
            _influx_client: influx_client,
            _hf_storage: hf_storage,
            metrics,
            connection_manager,
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

        // Check connection limits
        if !self.connection_manager.can_accept_connection().await {
            warn!("Connection limit reached, rejecting client: {}", peer_addr);
            return Err(Status::resource_exhausted(
                "Maximum connection limit reached",
            ));
        }

        // Increment connections
        if let Err(e) = self
            .connection_manager
            .increment_connections(self.metrics.as_ref())
            .await
        {
            warn!("Connection limit reached, rejecting client: {}", peer_addr);
            return Err(Status::resource_exhausted(e.to_string()));
        }

        let mut in_stream = request.into_inner();
        let (_tx, rx) = mpsc::channel(128);

        let _subscription_manager = self.subscription_manager.clone();
        let connection_manager = self.connection_manager.clone();
        let metrics = self.metrics.clone();

        // Spawn a task to handle incoming requests from this client
        tokio::spawn(async move {
            let client_id = uuid::Uuid::new_v4().to_string();
            info!("Client session started: {} ({})", client_id, peer_addr);

            // Handle incoming messages
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(req) => {
                        match req.request {
                            Some(crate::proto::subscription_request::Request::Subscribe(sub)) => {
                                info!("Client {} subscribing to {:?}", client_id, sub.symbols);
                                // TODO: Implement subscription logic properly
                                // subscription_manager.subscribe(&client_id, &sub.symbol).await;

                                // Dummy response for now since we need to return MarketDataMessage
                                // In a real implementation, this would come from the subscription manager
                            }
                            Some(crate::proto::subscription_request::Request::Unsubscribe(
                                unsub,
                            )) => {
                                info!(
                                    "Client {} unsubscribing from {:?}",
                                    client_id, unsub.symbols
                                );
                                // TODO: Implement unsubscribe logic properly
                                // subscription_manager.unsubscribe(&client_id, &unsub.symbol).await;
                            }
                            Some(crate::proto::subscription_request::Request::Heartbeat(_)) => {
                                // Heartbeat logic
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
            // Cleanup subscriptions
            // subscription_manager.remove_client(&client_id).await;

            // Release connection slot
            connection_manager
                .decrement_connections(std::time::Duration::from_secs(0), metrics.as_ref())
                .await;
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

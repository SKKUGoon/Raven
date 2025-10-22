// Control Service gRPC Implementation
// "The Hand of the King's gRPC interface for commanding the realm's data collection"

use crate::control::manager::CollectorManager;
use crate::exchanges::types::Exchange;
use crate::proto::control_service_server::ControlService;
use crate::proto::{
    CollectionInfo, ListCollectionsRequest, ListCollectionsResponse, StartCollectionRequest,
    StartCollectionResponse, StopCollectionRequest, StopCollectionResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

/// Implementation of the ControlService gRPC interface
pub struct ControlServiceImpl {
    collector_manager: Arc<CollectorManager>,
}

impl ControlServiceImpl {
    /// Create a new ControlServiceImpl
    pub fn new(collector_manager: Arc<CollectorManager>) -> Self {
        Self { collector_manager }
    }

    /// Parse exchange string to Exchange enum
    fn parse_exchange(exchange_str: &str) -> Result<Exchange, Status> {
        match exchange_str.to_lowercase().as_str() {
            "binance_futures" | "binance-futures" => Ok(Exchange::BinanceFutures),
            "binance_spot" | "binance-spot" => Ok(Exchange::BinanceSpot),
            _ => Err(Status::invalid_argument(format!(
                "Unsupported exchange: {exchange_str}. Supported exchanges: binance_futures, binance_spot",
            ))),
        }
    }

    /// Convert Exchange enum to string
    fn exchange_to_string(exchange: &Exchange) -> String {
        match exchange {
            Exchange::BinanceFutures => "binance_futures".to_string(),
            Exchange::BinanceSpot => "binance_spot".to_string(),
        }
    }

    /// Convert CollectionInfo to proto CollectionInfo
    fn convert_collection_info(info: &crate::control::manager::CollectionInfo) -> CollectionInfo {
        CollectionInfo {
            exchange: Self::exchange_to_string(&info.exchange),
            symbol: info.symbol.clone(),
            collection_id: info.collection_id.clone(),
            started_at: info.started_at,
            status: info.status.clone(),
            data_types: info.data_types.clone(),
        }
    }
}

#[tonic::async_trait]
impl ControlService for ControlServiceImpl {
    /// Start collecting data for a specific exchange-symbol pair
    async fn start_collection(
        &self,
        request: Request<StartCollectionRequest>,
    ) -> Result<Response<StartCollectionResponse>, Status> {
        let req = request.into_inner();
        let exchange_str = req.exchange;
        let symbol = req.symbol;

        info!(
            exchange = %exchange_str,
            symbol = %symbol,
            "Received start collection request"
        );

        // Parse exchange
        let exchange = Self::parse_exchange(&exchange_str)?;

        // Validate symbol (basic validation)
        if symbol.is_empty() {
            return Err(Status::invalid_argument("Symbol cannot be empty"));
        }

        // Start collection
        match self
            .collector_manager
            .start_collection(exchange, symbol.clone())
            .await
        {
            Ok(collection_id) => {
                info!(
                    collection_id = %collection_id,
                    exchange = %exchange_str,
                    symbol = %symbol,
                    "Collection started successfully"
                );

                let response = StartCollectionResponse {
                    success: true,
                    message: format!("Successfully started collection for {exchange_str}:{symbol}"),
                    collection_id,
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                error!(
                    exchange = %exchange_str,
                    symbol = %symbol,
                    error = %e,
                    "Failed to start collection"
                );

                let response = StartCollectionResponse {
                    success: false,
                    message: format!("Failed to start collection: {e}"),
                    collection_id: String::new(),
                };

                Ok(Response::new(response))
            }
        }
    }

    /// Stop collecting data for a specific exchange-symbol pair
    async fn stop_collection(
        &self,
        request: Request<StopCollectionRequest>,
    ) -> Result<Response<StopCollectionResponse>, Status> {
        let req = request.into_inner();
        let exchange_str = req.exchange;
        let symbol = req.symbol;

        info!(
            exchange = %exchange_str,
            symbol = %symbol,
            "Received stop collection request"
        );

        // Parse exchange
        let exchange = Self::parse_exchange(&exchange_str)?;

        // Validate symbol
        if symbol.is_empty() {
            return Err(Status::invalid_argument("Symbol cannot be empty"));
        }

        // Stop collection
        match self
            .collector_manager
            .stop_collection(exchange, symbol.clone())
            .await
        {
            Ok(_) => {
                info!(
                    exchange = %exchange_str,
                    symbol = %symbol,
                    "Collection stopped successfully"
                );

                let response = StopCollectionResponse {
                    success: true,
                    message: format!("Successfully stopped collection for {exchange_str}:{symbol}"),
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                warn!(
                    exchange = %exchange_str,
                    symbol = %symbol,
                    error = %e,
                    "Failed to stop collection"
                );

                let response = StopCollectionResponse {
                    success: false,
                    message: format!("Failed to stop collection: {e}"),
                };

                Ok(Response::new(response))
            }
        }
    }

    /// List all active collections
    async fn list_collections(
        &self,
        _request: Request<ListCollectionsRequest>,
    ) -> Result<Response<ListCollectionsResponse>, Status> {
        info!("Received list collections request");

        let collections = self.collector_manager.list_collections();
        let proto_collections: Vec<CollectionInfo> = collections
            .iter()
            .map(Self::convert_collection_info)
            .collect();

        info!(
            count = collections.len(),
            "Returning {} active collections",
            collections.len()
        );

        let response = ListCollectionsResponse {
            collections: proto_collections,
        };

        Ok(Response::new(response))
    }
}

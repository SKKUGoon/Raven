use crate::proto::control_server::Control;
use crate::proto::market_data_server::MarketData;
use crate::proto::{
    market_data_message, CollectionInfo, ControlRequest, ControlResponse, ListRequest,
    ListResponse, MarketDataMessage, MarketDataRequest, StopAllRequest, StopAllResponse,
};
use crate::service::StreamKey;
use async_trait::async_trait;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::service::BinanceFuturesKlinesService;

#[async_trait]
impl Control for BinanceFuturesKlinesService {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        if req.data_type != crate::proto::DataType::Candle as i32 {
            return Err(Status::invalid_argument(
                "This service only supports data_type=CANDLE",
            ));
        }
        if !req.venue.trim().is_empty() && req.venue.to_uppercase() != self.venue() {
            return Err(Status::invalid_argument(format!(
                "Invalid venue '{}'; expected {}",
                req.venue,
                self.venue()
            )));
        }
        self.start_symbol(&req.symbol).await?;
        Ok(Response::new(ControlResponse {
            success: true,
            message: format!("Started kline candles for {}", req.symbol),
        }))
    }

    async fn stop_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        if req.data_type != crate::proto::DataType::Candle as i32 {
            return Err(Status::invalid_argument(
                "This service only supports data_type=CANDLE",
            ));
        }
        self.stop_symbol(&req.symbol).await?;
        Ok(Response::new(ControlResponse {
            success: true,
            message: format!("Stopped kline candles for {}", req.symbol),
        }))
    }

    async fn list_collections(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let collections = self
            .active_keys()
            .into_iter()
            .map(|k| CollectionInfo {
                symbol: k.symbol,
                status: "active".to_string(),
                subscriber_count: 0,
            })
            .collect();
        Ok(Response::new(ListResponse { collections }))
    }

    async fn stop_all_collections(
        &self,
        _request: Request<StopAllRequest>,
    ) -> Result<Response<StopAllResponse>, Status> {
        let keys: Vec<StreamKey> = self.active_keys();
        for k in keys {
            let _ = self.stop_symbol(&k.symbol).await;
        }
        Ok(Response::new(StopAllResponse {
            success: true,
            message: "Stopped all kline collections".to_string(),
        }))
    }
}

#[async_trait]
impl MarketData for BinanceFuturesKlinesService {
    type SubscribeStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<MarketDataRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        if req.data_type != crate::proto::DataType::Candle as i32 {
            return Err(Status::invalid_argument(
                "This service only supports data_type=CANDLE",
            ));
        }
        if !req.venue.trim().is_empty() && req.venue.to_uppercase() != self.venue() {
            return Err(Status::invalid_argument(format!(
                "Invalid venue '{}'; expected {}",
                req.venue,
                self.venue()
            )));
        }

        let symbol = req.symbol.trim().to_uppercase();
        let key = self.key_for_symbol(&symbol);
        if !self.is_active(&key) {
            return Err(Status::failed_precondition(format!(
                "Stream not started for {key}. Call Control.StartCollection first."
            )));
        }

        let rx = self.tx().subscribe();
        let stream = tokio_stream::StreamExt::filter_map(BroadcastStream::new(rx), move |item| {
            let symbol = symbol.clone();
            match item {
                Ok(Ok(m)) => {
                    // Filter: pass through only candles for this symbol.
                    match &m.data {
                        Some(market_data_message::Data::Candle(c)) if c.symbol == symbol => {
                            Some(Ok(m))
                        }
                        _ => None,
                    }
                }
                Ok(Err(e)) => Some(Err(e)),
                Err(_) => Some(Err(Status::internal("Stream lagged"))),
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }
}

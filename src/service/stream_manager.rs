use dashmap::DashMap;
use futures_util::StreamExt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::info;

use crate::proto::control_server::Control;
use crate::proto::market_data_server::MarketData;
use crate::proto::{
    CollectionInfo, ControlRequest, ControlResponse, ListRequest, ListResponse, MarketDataMessage,
    MarketDataRequest, StopAllRequest, StopAllResponse,
};

pub struct StreamManager<F> {
    channels: Arc<DashMap<String, broadcast::Sender<Result<MarketDataMessage, Status>>>>,
    tasks: Arc<DashMap<String, JoinHandle<()>>>,
    factory: Arc<F>,
}

impl<F> Clone for StreamManager<F> {
    fn clone(&self) -> Self {
        Self {
            channels: self.channels.clone(),
            tasks: self.tasks.clone(),
            factory: self.factory.clone(),
        }
    }
}

impl<F, Fut> StreamManager<F>
where
    F: Fn(String, broadcast::Sender<Result<MarketDataMessage, Status>>) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    pub fn new(factory: F) -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
            tasks: Arc::new(DashMap::new()),
            factory: Arc::new(factory),
        }
    }

    pub async fn ensure_stream(
        &self,
        symbol: &str,
    ) -> broadcast::Sender<Result<MarketDataMessage, Status>> {
        if let Some(entry) = self.channels.get(symbol) {
            return entry.value().clone();
        }

        let (tx, _) = broadcast::channel(100);
        self.channels.insert(symbol.to_string(), tx.clone());

        // Spawn WS task
        let symbol_clone = symbol.to_string();
        let tx_clone = tx.clone();
        let factory = self.factory.clone();

        let handle = tokio::spawn(async move {
            (factory)(symbol_clone, tx_clone).await;
        });
        self.tasks.insert(symbol.to_string(), handle);

        tx
    }
}

#[tonic::async_trait]
impl<F, Fut> Control for StreamManager<F>
where
    F: Fn(String, broadcast::Sender<Result<MarketDataMessage, Status>>) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        info!("Control: Start collection for {symbol}");
        self.ensure_stream(&symbol).await;

        Ok(Response::new(ControlResponse {
            success: true,
            message: format!("Started collection for {symbol}"),
        }))
    }

    async fn stop_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        if let Some((_, handle)) = self.tasks.remove(&symbol) {
            handle.abort();
            self.channels.remove(&symbol);
            info!("Control: Stopped collection for {symbol}");
            Ok(Response::new(ControlResponse {
                success: true,
                message: format!("Stopped collection for {symbol}"),
            }))
        } else {
            Ok(Response::new(ControlResponse {
                success: false,
                message: format!("No collection found for {symbol}"),
            }))
        }
    }

    async fn list_collections(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let collections = self
            .channels
            .iter()
            .map(|entry| CollectionInfo {
                symbol: entry.key().clone(),
                status: "active".to_string(),
                subscriber_count: entry.value().receiver_count() as i32,
            })
            .collect();

        Ok(Response::new(ListResponse { collections }))
    }

    async fn stop_all_collections(
        &self,
        _request: Request<StopAllRequest>,
    ) -> Result<Response<StopAllResponse>, Status> {
        for entry in self.tasks.iter() {
            entry.value().abort();
        }
        self.tasks.clear();
        self.channels.clear();
        info!("Control: Stopped all collections");
        Ok(Response::new(StopAllResponse {
            success: true,
            message: "All collections stopped".to_string(),
        }))
    }
}

#[tonic::async_trait]
impl<F, Fut> MarketData for StreamManager<F>
where
    F: Fn(String, broadcast::Sender<Result<MarketDataMessage, Status>>) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<MarketDataRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        info!("Client subscribed to {}", symbol);
        let tx = self.ensure_stream(&symbol).await;
        let rx = tx.subscribe();

        let stream = BroadcastStream::new(rx).map(|item| match item {
            Ok(msg) => msg,
            Err(_) => Err(Status::internal("Stream lagged")),
        });

        Ok(Response::new(Box::pin(stream)))
    }
}

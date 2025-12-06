use dashmap::DashMap;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::proto::control_server::Control;
use crate::proto::market_data_server::MarketData;
use crate::proto::{
    CollectionInfo, ControlRequest, ControlResponse, ListRequest, ListResponse, MarketDataMessage,
    MarketDataRequest, StopAllRequest, StopAllResponse,
};

/// Trait for stream workers that handle the actual data collection.
#[tonic::async_trait]
pub trait StreamWorker: Send + Sync + 'static {
    /// Run the worker for a specific symbol.
    /// This method should run indefinitely unless an error occurs or it is cancelled.
    async fn run(&self, symbol: String, tx: broadcast::Sender<Result<MarketDataMessage, Status>>);
}

pub struct StreamManager<W: StreamWorker + ?Sized> {
    channels: Arc<DashMap<String, broadcast::Sender<Result<MarketDataMessage, Status>>>>,
    tasks: Arc<DashMap<String, JoinHandle<()>>>,
    worker: Arc<W>,
    channel_capacity: usize,
    prune_on_no_subscribers: bool,
}

impl<W: StreamWorker + ?Sized> Clone for StreamManager<W> {
    fn clone(&self) -> Self {
        Self {
            channels: self.channels.clone(),
            tasks: self.tasks.clone(),
            worker: self.worker.clone(),
            channel_capacity: self.channel_capacity,
            prune_on_no_subscribers: self.prune_on_no_subscribers,
        }
    }
}

impl<W: StreamWorker + ?Sized> StreamManager<W> {
    pub fn new(worker: Arc<W>, channel_capacity: usize, prune_on_no_subscribers: bool) -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
            tasks: Arc::new(DashMap::new()),
            worker,
            channel_capacity,
            prune_on_no_subscribers,
        }
    }

    pub async fn ensure_stream(
        &self,
        symbol: &str,
    ) -> broadcast::Sender<Result<MarketDataMessage, Status>> {
        if let Some(entry) = self.channels.get(symbol) {
            return entry.value().clone();
        }

        let (tx, _) = broadcast::channel(self.channel_capacity);
        self.channels.insert(symbol.to_string(), tx.clone());

        // Spawn supervisor task
        let symbol_clone = symbol.to_string();
        let tx_clone = tx.clone();
        let worker = self.worker.clone();
        let channels = self.channels.clone();
        let tasks = self.tasks.clone();
        let prune = self.prune_on_no_subscribers;

        let handle = tokio::spawn(async move {
            loop {
                // Run the worker
                worker.run(symbol_clone.clone(), tx_clone.clone()).await;

                // Worker exited. Check if we should restart or clean up.
                // If prune is enabled and no subscribers left, we stop.
                if prune && tx_clone.receiver_count() == 0 {
                    info!(
                        "Stream for {} ended (no subscribers). Cleaning up.",
                        symbol_clone
                    );
                    channels.remove(&symbol_clone);
                    tasks.remove(&symbol_clone);
                    break;
                }

                // Otherwise (subscribers exist OR prune is disabled), restart with backoff.
                warn!(
                    "Stream worker for {} exited unexpectedly. Restarting in 5s...",
                    symbol_clone
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });

        self.tasks.insert(symbol.to_string(), handle);

        tx
    }
}

#[tonic::async_trait]
impl<W: StreamWorker + ?Sized> Control for StreamManager<W> {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();
        let exchange = req.exchange.to_uppercase();

        let key = if exchange.is_empty() {
            symbol.clone()
        } else {
            format!("{symbol}:{exchange}")
        };

        info!("Control: Start collection for {key}");
        self.ensure_stream(&key).await;

        Ok(Response::new(ControlResponse {
            success: true,
            message: format!("Started collection for {key}"),
        }))
    }

    async fn stop_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();
        let exchange = req.exchange.to_uppercase();

        let key = if exchange.is_empty() {
            symbol.clone()
        } else {
            format!("{symbol}:{exchange}")
        };

        if let Some((_, handle)) = self.tasks.remove(&key) {
            handle.abort();
            self.channels.remove(&key);
            info!("Control: Stopped collection for {key}");
            Ok(Response::new(ControlResponse {
                success: true,
                message: format!("Stopped collection for {key}"),
            }))
        } else {
            Ok(Response::new(ControlResponse {
                success: false,
                message: format!("No collection found for {key}"),
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
impl<W: StreamWorker + ?Sized> MarketData for StreamManager<W> {
    type SubscribeStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

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

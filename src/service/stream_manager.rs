use dashmap::mapref::entry::Entry;
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
use crate::service::stream_key::StreamKey;

/// Trait for stream workers that handle the actual data collection.
#[tonic::async_trait]
pub trait StreamWorker: Send + Sync + 'static {
    /// Run the worker for a specific symbol.
    /// This method should run indefinitely unless an error occurs or it is cancelled.
    async fn run(&self, key: StreamKey, tx: broadcast::Sender<Result<MarketDataMessage, Status>>);
}

pub struct StreamManager<W: StreamWorker + ?Sized> {
    channels: Arc<DashMap<StreamKey, broadcast::Sender<Result<MarketDataMessage, Status>>>>,
    tasks: Arc<DashMap<StreamKey, JoinHandle<()>>>,
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
        key: StreamKey,
    ) -> broadcast::Sender<Result<MarketDataMessage, Status>> {
        match self.channels.entry(key.clone()) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let (tx, _) = broadcast::channel(self.channel_capacity);
                entry.insert(tx.clone());

                // Spawn supervisor task
                let key_clone = key.clone();
                let tx_clone = tx.clone();
                let worker = self.worker.clone();
                let channels = self.channels.clone();
                let tasks = self.tasks.clone();
                let prune = self.prune_on_no_subscribers;

                let handle = tokio::spawn(async move {
                    loop {
                        // Run the worker
                        worker.run(key_clone.clone(), tx_clone.clone()).await;

                        // Worker exited. Check if we should restart or clean up.
                        // If prune is enabled and no subscribers left, we *may* stop.
                        // However, add a short grace period to avoid thrashing with retrying clients
                        // ("wire first, subscribe later") where consumers may retry Subscribe every few seconds.
                        if prune && tx_clone.receiver_count() == 0 {
                            const NO_SUBSCRIBER_GRACE: Duration = Duration::from_secs(5);
                            tokio::time::sleep(NO_SUBSCRIBER_GRACE).await;

                            if tx_clone.receiver_count() == 0 {
                                info!(
                                    "Stream for {} ended (no subscribers). Cleaning up.",
                                    key_clone
                                );
                                channels.remove(&key_clone);
                                tasks.remove(&key_clone);
                                break;
                            }
                        }

                        // Otherwise (subscribers exist OR prune is disabled), restart with backoff.
                        warn!(
                            "Stream worker for {} exited unexpectedly. Restarting in 5s...",
                            key_clone
                        );
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                });

                self.tasks.insert(key, handle);

                tx
            }
        }
    }
}

#[tonic::async_trait]
impl<W: StreamWorker + ?Sized> Control for StreamManager<W> {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let key = StreamKey::from_control_with_datatype(&req.symbol, &req.venue, req.data_type);

        info!("Control: Start collection for {key}");
        self.ensure_stream(key.clone()).await;

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
        let key = StreamKey::from_control_with_datatype(&req.symbol, &req.venue, req.data_type);

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
                symbol: entry.key().to_string(),
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
        let key = StreamKey::from_market_request(&req.symbol, &req.venue, req.data_type);

        let tx = self
            .channels
            .get(&key)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| {
                // Note: don't log this at info-level; retrying clients can spam.
                Status::failed_precondition(format!(
                    "Stream not started for {key}. Call Control.StartCollection first."
                ))
            })?;
        info!("Client subscribed (key: {key})");
        let rx = tx.subscribe();

        let stream = BroadcastStream::new(rx).map(|item| match item {
            Ok(msg) => msg,
            Err(_) => Err(Status::internal("Stream lagged")),
        });

        Ok(Response::new(Box::pin(stream)))
    }
}

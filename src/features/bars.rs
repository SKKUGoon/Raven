use dashmap::DashMap;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, register_int_gauge, IntCounterVec, IntGauge};
use crate::proto::control_server::Control;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::market_data_server::MarketData;
use crate::proto::{
    market_data_message, Candle, ControlRequest, ControlResponse, DataType, ListRequest,
    ListResponse, MarketDataMessage, MarketDataRequest, StopAllRequest, StopAllResponse,
};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{error, info};

lazy_static! {
    static ref BARS_GENERATED: IntCounterVec = register_int_counter_vec!(
        "raven_bars_generated_total",
        "Total number of bars generated",
        &["symbol"]
    )
    .unwrap();
    static ref ACTIVE_AGGREGATIONS: IntGauge = register_int_gauge!(
        "raven_bars_active_aggregations",
        "Number of active bar aggregation tasks"
    )
    .unwrap();
}

#[derive(Clone, Debug)]
struct Bar {
    symbol: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    start_time: i64,
}

impl Bar {
    fn new(symbol: String, price: f64, quantity: f64, timestamp: i64) -> Self {
        Self {
            symbol,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: quantity,
            start_time: timestamp - (timestamp % 60000), // Snap to minute
        }
    }

    fn update(&mut self, price: f64, quantity: f64, timestamp: i64) -> Option<Bar> {
        let period_start = timestamp - (timestamp % 60000);

        if period_start > self.start_time {
            // New bar started, return completed bar
            let closed_bar = self.clone();

            // Reset current bar
            self.start_time = period_start;
            self.open = price;
            self.high = price;
            self.low = price;
            self.close = price;
            self.volume = quantity;

            Some(closed_bar)
        } else {
            // Update current bar
            self.high = f64::max(self.high, price);
            self.low = f64::min(self.low, price);
            self.close = price;
            self.volume += quantity;
            None
        }
    }

    fn to_proto(&self) -> Candle {
        Candle {
            symbol: self.symbol.clone(),
            timestamp: self.start_time,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
            interval: "1m".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct BarService {
    upstream_url: String,
    // Symbol -> Broadcast Sender for Candles
    channels: Arc<DashMap<String, broadcast::Sender<Result<MarketDataMessage, Status>>>>,
    // Symbol -> Aggregation Task Handle
    tasks: Arc<DashMap<String, JoinHandle<()>>>,
}

impl BarService {
    pub fn new(upstream_url: String) -> Self {
        Self {
            upstream_url,
            channels: Arc::new(DashMap::new()),
            tasks: Arc::new(DashMap::new()),
        }
    }

    async fn ensure_stream(
        &self,
        symbol: &str,
    ) -> broadcast::Sender<Result<MarketDataMessage, Status>> {
        if let Some(entry) = self.channels.get(symbol) {
            return entry.value().clone();
        }

        let (tx, _) = broadcast::channel(100);
        self.channels.insert(symbol.to_string(), tx.clone());

        // Start aggregation if not running
        if !self.tasks.contains_key(symbol) {
            let symbol_clone = symbol.to_string();
            let tx_clone = tx.clone();
            let upstream = self.upstream_url.clone();

            let handle = tokio::spawn(async move {
                run_aggregation(upstream, symbol_clone, tx_clone).await;
            });
            self.tasks.insert(symbol.to_string(), handle);
        }

        tx
    }
}

async fn run_aggregation(
    upstream_url: String,
    symbol: String,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
) {
    info!("Starting bar aggregation for {}", symbol);
    ACTIVE_AGGREGATIONS.inc();

    let mut client = match MarketDataClient::connect(upstream_url).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to upstream: {}", e);
            ACTIVE_AGGREGATIONS.dec();
            return;
        }
    };

    let request = Request::new(MarketDataRequest {
        symbol: symbol.clone(),
        data_type: DataType::Trade as i32,
    });

    let mut stream = match client.subscribe(request).await {
        Ok(res) => res.into_inner(),
        Err(e) => {
            error!("Failed to subscribe to trades: {}", e);
            ACTIVE_AGGREGATIONS.dec();
            return;
        }
    };

    let mut current_bar: Option<Bar> = None;

    while let Ok(Some(msg)) = stream.message().await {
        if let Some(market_data_message::Data::Trade(trade)) = msg.data {
            if let Some(bar) = &mut current_bar {
                if let Some(completed) = bar.update(trade.price, trade.quantity, trade.timestamp) {
                    // Emit completed bar
                    BARS_GENERATED.with_label_values(&[&symbol]).inc();
                    let candle_msg = MarketDataMessage {
                        exchange: "raven_bars".to_string(),
                        data: Some(market_data_message::Data::Candle(completed.to_proto())),
                    };
                    if tx.send(Ok(candle_msg)).is_err() {
                        break;
                    }
                }
            } else {
                current_bar = Some(Bar::new(
                    symbol.clone(),
                    trade.price,
                    trade.quantity,
                    trade.timestamp,
                ));
            }
        }
    }
    info!("Aggregation task ended for {}", symbol);
    ACTIVE_AGGREGATIONS.dec();
}

#[tonic::async_trait]
impl Control for BarService {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        info!("Control: Start bars for {}", symbol);
        self.ensure_stream(&symbol).await;

        Ok(Response::new(ControlResponse {
            success: true,
            message: format!("Started bars for {symbol}"),
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
            info!("Control: Stopped bars for {}", symbol);
            Ok(Response::new(ControlResponse {
                success: true,
                message: format!("Stopped bars for {symbol}"),
            }))
        } else {
            Ok(Response::new(ControlResponse {
                success: false,
                message: format!("No bars task found for {symbol}"),
            }))
        }
    }

    async fn list_collections(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        use crate::proto::CollectionInfo;
        let collections = self
            .tasks
            .iter()
            .map(|entry| CollectionInfo {
                symbol: entry.key().clone(),
                status: "active".to_string(),
                subscriber_count: if let Some(ch) = self.channels.get(entry.key()) {
                    ch.value().receiver_count() as i32
                } else {
                    0
                },
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
        info!("Control: Stopped all bars");
        Ok(Response::new(StopAllResponse {
            success: true,
            message: "All bars stopped".to_string(),
        }))
    }
}

#[tonic::async_trait]
impl MarketData for BarService {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<MarketDataRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        info!("Client subscribed to bars {}", symbol);
        let tx = self.ensure_stream(&symbol).await;
        let rx = tx.subscribe();

        let stream = BroadcastStream::new(rx).map(|item| match item {
            Ok(msg) => msg,
            Err(_) => Err(Status::internal("Stream lagged")),
        });

        Ok(Response::new(Box::pin(stream)))
    }
}


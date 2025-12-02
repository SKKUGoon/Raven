use chrono::{DateTime, TimeZone, Utc};
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
    static ref TIBS_GENERATED: IntCounterVec = register_int_counter_vec!(
        "raven_tibs_generated_total",
        "Total number of TIBs generated",
        &["symbol"]
    )
    .unwrap();
    static ref ACTIVE_TIB_AGGREGATIONS: IntGauge = register_int_gauge!(
        "raven_tibs_active_aggregations",
        "Number of active TIB aggregation tasks"
    )
    .unwrap();
}

/// Direction of tick imbalance
#[derive(Clone, Copy, Debug)]
pub enum TickDirection {
    Buy,
    Sell,
}

impl TickDirection {
    #[inline]
    pub fn to_i32(&self) -> i32 {
        match self {
            TickDirection::Buy => 1,
            TickDirection::Sell => -1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TickImbalanceBar {
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub trade_size: f64,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub tick_size: u32,
    pub theta: i32,
    pub bar_open: bool,
}

impl TickImbalanceBar {
    pub fn new(
        symbol: String,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Self {
        Self {
            symbol,
            open: price,
            high: price,
            low: price,
            close: price,
            trade_size: volume,
            open_time: ts,
            close_time: ts,
            tick_size: 1,
            theta: b_t.to_i32(),
            bar_open: true,
        }
    }

    fn update(&mut self, price: f64, volume: f64, b_t: TickDirection, ts: DateTime<Utc>) {
        if self.bar_open {
            self.high = price.max(self.high);
            self.low = price.min(self.low);
            self.close = price;
            self.trade_size += volume;
            self.close_time = ts;
            self.tick_size += 1;
            self.theta += b_t.to_i32();
        }
    }

    fn close(&mut self) {
        self.bar_open = false;
    }

    fn p_buy(&self) -> f64 {
        // This is a simplified approximation of p_buy based on current state
        // In a real implementation, we'd track buy_ticks explicitly if needed for logic
        // For now, assuming theta roughly correlates to buy_ticks - sell_ticks
        // buy_ticks + sell_ticks = tick_size
        // buy_ticks - sell_ticks = theta
        // 2 * buy_ticks = tick_size + theta
        // buy_ticks = (tick_size + theta) / 2
        let buy_ticks = (self.tick_size as i32 + self.theta) as f64 / 2.0;
        buy_ticks / self.tick_size as f64
    }

    fn to_proto(&self) -> Candle {
        Candle {
            symbol: self.symbol.clone(),
            timestamp: self.open_time.timestamp_millis(),
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.trade_size,
            interval: "tib".to_string(),
        }
    }
}

pub struct TickImbalanceState {
    pub threshold: f64,
    pub size_ewma: f64,
    pub imbl_ewma: f64,
    pub alpha_size: f64,
    pub alpha_imbl: f64,
    pub size_boundary: (f64, f64),
    pub current_bar: Option<TickImbalanceBar>,
}

impl TickImbalanceState {
    pub fn new(
        initial_size: f64,
        initial_p_buy: f64,
        alpha_size: f64,
        alpha_imbl: f64,
        size_boundary: (f64, f64),
    ) -> Self {
        let size_ewma = initial_size;
        let imbl_ewma = 2.0 * initial_p_buy - 1.0;
        let threshold = size_ewma * imbl_ewma;

        Self {
            threshold,
            size_ewma,
            imbl_ewma,
            alpha_size,
            alpha_imbl,
            size_boundary,
            current_bar: None,
        }
    }

    fn bound(val: f64, min: f64, max: f64) -> f64 {
        val.max(min).min(max)
    }

    fn on_tick(
        &mut self,
        symbol: String,
        price: f64,
        volume: f64,
        b_t: TickDirection,
        ts: DateTime<Utc>,
    ) -> Option<TickImbalanceBar> {
        if self.current_bar.is_none() {
            self.current_bar = Some(TickImbalanceBar::new(symbol, price, volume, b_t, ts));
            return None;
        }

        let bar = self.current_bar.as_mut().unwrap();
        bar.update(price, volume, b_t, ts);

        if (bar.theta as f64).abs() > self.threshold.abs() {
            bar.close();
            let closed = self.current_bar.take().unwrap();

            let new_imbl = 2.0 * closed.p_buy() - 1.0;
            self.imbl_ewma = self.alpha_imbl * new_imbl + (1.0 - self.alpha_imbl) * self.imbl_ewma;

            let new_size = closed.tick_size as f64;
            self.size_ewma = self.alpha_size * new_size + (1.0 - self.alpha_size) * self.size_ewma;
            self.size_ewma =
                Self::bound(self.size_ewma, self.size_boundary.0, self.size_boundary.1);

            self.threshold = self.size_ewma * self.imbl_ewma;

            return Some(closed);
        }

        None
    }
}

#[derive(Clone)]
pub struct TibsService {
    upstream_url: String,
    channels: Arc<DashMap<String, broadcast::Sender<Result<MarketDataMessage, Status>>>>,
    tasks: Arc<DashMap<String, JoinHandle<()>>>,
}

impl TibsService {
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

        if !self.tasks.contains_key(symbol) {
            let symbol_clone = symbol.to_string();
            let tx_clone = tx.clone();
            let upstream = self.upstream_url.clone();

            let handle = tokio::spawn(async move {
                run_tib_aggregation(upstream, symbol_clone, tx_clone).await;
            });
            self.tasks.insert(symbol.to_string(), handle);
        }

        tx
    }
}

async fn run_tib_aggregation(
    upstream_url: String,
    symbol: String,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
) {
    info!("Starting TIB aggregation for {}", symbol);
    ACTIVE_TIB_AGGREGATIONS.inc();

    let mut client = match MarketDataClient::connect(upstream_url).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to upstream: {}", e);
            ACTIVE_TIB_AGGREGATIONS.dec();
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
            ACTIVE_TIB_AGGREGATIONS.dec();
            return;
        }
    };

    // Initialize State with defaults
    // In production, these should come from config or historical data analysis
    let mut state = TickImbalanceState::new(
        100.0,          // Initial size
        0.7,            // Initial p_buy (strong signal)
        0.1,            // Alpha size
        0.1,            // Alpha imbl
        (50.0, 1000.0), // Size boundaries
    );

    while let Ok(Some(msg)) = stream.message().await {
        if let Some(market_data_message::Data::Trade(trade)) = msg.data {
            let direction = match trade.side.as_str() {
                "buy" => TickDirection::Buy,
                _ => TickDirection::Sell,
            };

            let ts = Utc.timestamp_millis_opt(trade.timestamp).unwrap();

            if let Some(closed_bar) =
                state.on_tick(symbol.clone(), trade.price, trade.quantity, direction, ts)
            {
                TIBS_GENERATED.with_label_values(&[&symbol]).inc();
                let msg = MarketDataMessage {
                    exchange: "raven_tibs".to_string(),
                    data: Some(market_data_message::Data::Candle(closed_bar.to_proto())),
                };
                if tx.send(Ok(msg)).is_err() {
                    break;
                }
            }
        }
    }
    info!("TIB aggregation task ended for {}", symbol);
    ACTIVE_TIB_AGGREGATIONS.dec();
}

#[tonic::async_trait]
impl Control for TibsService {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        info!("Control: Start TIBs for {}", symbol);
        self.ensure_stream(&symbol).await;

        Ok(Response::new(ControlResponse {
            success: true,
            message: format!("Started TIBs for {symbol}"),
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
            info!("Control: Stopped TIBs for {}", symbol);
            Ok(Response::new(ControlResponse {
                success: true,
                message: format!("Stopped TIBs for {symbol}"),
            }))
        } else {
            Ok(Response::new(ControlResponse {
                success: false,
                message: format!("No TIBs task found for {symbol}"),
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
        info!("Control: Stopped all TIBs");
        Ok(Response::new(StopAllResponse {
            success: true,
            message: "All TIBs stopped".to_string(),
        }))
    }
}

#[tonic::async_trait]
impl MarketData for TibsService {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<MarketDataRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase();

        info!("Client subscribed to TIBs {}", symbol);
        let tx = self.ensure_stream(&symbol).await;
        let rx = tx.subscribe();

        let stream = BroadcastStream::new(rx).map(|item| match item {
            Ok(msg) => msg,
            Err(_) => Err(Status::internal("Stream lagged")),
        });

        Ok(Response::new(Box::pin(stream)))
    }
}


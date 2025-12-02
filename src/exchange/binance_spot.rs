use dashmap::DashMap;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, register_int_gauge, IntCounterVec, IntGauge};
use crate::proto::control_server::Control;
use crate::proto::market_data_server::MarketData;
use crate::proto::{
    market_data_message, ControlRequest, ControlResponse, ListRequest, ListResponse,
    MarketDataMessage, MarketDataRequest, StopAllRequest, StopAllResponse, Trade,
};
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{error, info};
use url::Url;

lazy_static! {
    static ref TRADES_PROCESSED: IntCounterVec = register_int_counter_vec!(
        "raven_binance_spot_trades_processed_total",
        "Total number of trades processed",
        &["symbol"]
    )
    .unwrap();
    static ref ACTIVE_CONNECTIONS: IntGauge = register_int_gauge!(
        "raven_binance_spot_active_connections",
        "Number of active WebSocket connections"
    )
    .unwrap();
}

#[derive(Clone)]
pub struct BinanceSpotService {
    // Map symbol -> Sender
    // We send Result<MarketDataMessage, Status> to match the gRPC stream requirement
    channels: Arc<DashMap<String, broadcast::Sender<Result<MarketDataMessage, Status>>>>,
    tasks: Arc<DashMap<String, JoinHandle<()>>>,
}

impl BinanceSpotService {
    pub fn new() -> Self {
        Self {
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

        // Spawn WS task
        let symbol_clone = symbol.to_string();
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            run_binance_ws(symbol_clone, tx_clone).await;
        });
        self.tasks.insert(symbol.to_string(), handle);

        tx
    }
}

impl Default for BinanceSpotService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl Control for BinanceSpotService {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.to_uppercase(); // Binance symbols are uppercase

        info!("Control: Start collection for {}", symbol);
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
        use crate::proto::CollectionInfo;

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
impl MarketData for BinanceSpotService {
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

async fn run_binance_ws(symbol: String, tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
    let stream_name = format!("{}@trade", symbol.to_lowercase());
    let url_str = format!("wss://stream.binance.com:9443/ws/{stream_name}");
    let url = Url::parse(&url_str).unwrap();

    info!("Connecting to Binance WS: {url}");

    match tokio_tungstenite::connect_async(url.to_string()).await {
        Ok((ws_stream, _)) => {
            info!("Connected to Binance for {symbol}");
            ACTIVE_CONNECTIONS.inc();
            let (_, mut read) = ws_stream.split();

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                        if let Some(trade) = parse_binance_trade(&text, &symbol) {
                            TRADES_PROCESSED.with_label_values(&[&symbol]).inc();
                            let msg = MarketDataMessage {
                                exchange: "binance_spot".to_string(),
                                data: Some(market_data_message::Data::Trade(trade)),
                            };
                            if tx.send(Ok(msg)).is_err() {
                                break; // No subscribers
                            }
                        }
                    }
                    Ok(tokio_tungstenite::tungstenite::Message::Ping(_)) => {
                        // Auto-pong handled by tungstenite usually, but good to know
                    }
                    Err(e) => {
                        error!("WS error for {}: {}", symbol, e);
                        break;
                    }
                    _ => {}
                }
            }
            ACTIVE_CONNECTIONS.dec();
        }
        Err(e) => {
            error!("Failed to connect to Binance for {}: {}", symbol, e);
        }
    }

    info!("WS task ended for {}", symbol);
}

fn parse_binance_trade(json: &str, symbol: &str) -> Option<Trade> {
    let v: Value = serde_json::from_str(json).ok()?;

    // {"e":"trade","E":123456789,"s":"BNBBTC","t":12345,"p":"0.001","q":"100","b":88,"a":50,"T":123456785,"m":true,"M":true}
    if v.get("e")?.as_str()? != "trade" {
        return None;
    }

    let price = v.get("p")?.as_str()?.parse().ok()?;
    let quantity = v.get("q")?.as_str()?.parse().ok()?;
    let timestamp = v.get("T")?.as_i64()?;
    let trade_id = v.get("t")?.as_u64()?.to_string();
    let is_buyer_maker = v.get("m")?.as_bool()?; // true = sell (maker is buyer), false = buy (maker is seller) -> taker is buyer

    let side = if is_buyer_maker {
        "sell".to_string()
    } else {
        "buy".to_string()
    };

    Some(Trade {
        symbol: symbol.to_string(),
        timestamp,
        price,
        quantity,
        side,
        trade_id,
    })
}


//! Binance Futures all-market liquidation stream: WS !forceOrder → Liquidation.
//!
//! Uses a single WebSocket connection to `wss://fstream.binance.com/ws/!forceOrder`
//! which pushes liquidation events for ALL perpetual symbols (snapshot per 1000ms).

use crate::proto::market_data_message::Data;
use crate::source::binance::constants::VENUE_BINANCE_FUTURES;
use crate::source::binance::control::{list_active_collections, stream_key};
use crate::source::binance::message::new_market_data_message;
use crate::source::binance::subscribe::filtered_broadcast_stream;
use crate::proto::{
    ControlRequest, ControlResponse, Liquidation, ListRequest, ListResponse, MarketDataMessage,
    MarketDataRequest, StopAllRequest, StopAllResponse,
};
use crate::service::{StreamDataType, StreamKey};
use async_trait::async_trait;
use dashmap::DashSet;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::proto::control_server::Control;
use crate::proto::market_data_server::MarketData;

const VENUE: &str = VENUE_BINANCE_FUTURES;
const PRODUCER: &str = "binance_futures_liquidations";
const WS_URL: &str = "wss://fstream.binance.com/ws/!forceOrder";
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const CONNECTION_LIFETIME: Duration = Duration::from_secs(23 * 3600 + 30 * 60);

#[derive(Clone)]
pub struct LiquidationService {
    active: Arc<DashSet<StreamKey>>,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
}

impl LiquidationService {
    pub fn new(channel_capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(channel_capacity);
        let active = Arc::new(DashSet::new());
        let service = Self { active, tx };
        service.spawn_ws();
        service
    }

    fn spawn_ws(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            run_force_order_stream(tx).await;
        });
    }

    fn key_for(symbol: &str) -> StreamKey {
        stream_key(symbol, VENUE, StreamDataType::Liquidation)
    }
}

async fn run_force_order_stream(
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
) {
    loop {
        info!("Binance liquidation stream: connecting to {WS_URL}");
        match tokio_tungstenite::connect_async(WS_URL).await {
            Ok((ws_stream, _)) => {
                info!("Binance liquidation stream: connected");
                let (_write, mut read) = ws_stream.split();
                let start = std::time::Instant::now();

                while let Some(msg) = read.next().await {
                    if start.elapsed() >= CONNECTION_LIFETIME {
                        info!("Binance liquidation stream: scheduled reconnection");
                        break;
                    }
                    match msg {
                        Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                            if let Some(data) = parse_force_order(&text) {
                                let msg = new_market_data_message(VENUE, PRODUCER, data);
                                let _ = tx.send(Ok(msg));
                            }
                        }
                        Ok(tokio_tungstenite::tungstenite::Message::Ping(_)) => {}
                        Err(e) => {
                            error!("Binance liquidation stream: WS error: {e}");
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("Binance liquidation stream: connect failed: {e}");
            }
        }
        warn!("Binance liquidation stream: reconnecting in {RETRY_INTERVAL:?}");
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
}

fn parse_force_order(text: &str) -> Option<Data> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("e")?.as_str()? != "forceOrder" {
        return None;
    }
    let o = v.get("o")?;
    let symbol = o.get("s")?.as_str()?.to_string();
    let side = o.get("S")?.as_str()?.to_lowercase();
    let price: f64 = o
        .get("ap")
        .or_else(|| o.get("p"))
        .and_then(|v| v.as_str()?.parse().ok())?;
    let quantity: f64 = o.get("q").and_then(|v| v.as_str()?.parse().ok())?;
    let timestamp = o.get("T").and_then(|v| v.as_i64()).unwrap_or(0);

    Some(Data::Liquidation(Liquidation {
        symbol,
        timestamp,
        side,
        price,
        quantity,
    }))
}

#[async_trait]
impl Control for LiquidationService {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let key = Self::key_for(&req.symbol);
        self.active.insert(key.clone());
        info!("{PRODUCER}: started collection for {key}");
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
        let key = Self::key_for(&req.symbol);
        self.active.remove(&key);
        Ok(Response::new(ControlResponse {
            success: true,
            message: format!("Stopped collection for {key}"),
        }))
    }

    async fn list_collections(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let collections = list_active_collections(&self.active);
        Ok(Response::new(ListResponse { collections }))
    }

    async fn stop_all_collections(
        &self,
        _request: Request<StopAllRequest>,
    ) -> Result<Response<StopAllResponse>, Status> {
        self.active.clear();
        Ok(Response::new(StopAllResponse {
            success: true,
            message: "Stopped all liquidation collections".to_string(),
        }))
    }
}

#[async_trait]
impl MarketData for LiquidationService {
    type SubscribeStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<MarketDataRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.trim().to_uppercase();
        let stream = filtered_broadcast_stream(&self.tx, symbol, |m, sym, wildcard| {
            matches!(&m.data, Some(Data::Liquidation(l)) if wildcard || l.symbol == sym)
        });
        Ok(Response::new(stream))
    }
}

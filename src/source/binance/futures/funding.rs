//! Binance Futures mark price stream: WS !markPrice@arr@1s → FundingRate.
//!
//! Uses a single WebSocket connection to `wss://fstream.binance.com/ws/!markPrice@arr@1s`
//! which pushes mark price + funding rate for ALL perpetual symbols every 1 second.
//! Replaces the previous REST polling approach to save API rate limit budget.

use crate::proto::market_data_message::Data;
use crate::proto::{
    ControlRequest, ControlResponse, FundingRate, ListRequest, ListResponse, MarketDataMessage,
    MarketDataRequest, StopAllRequest, StopAllResponse,
};
use crate::service::{StreamDataType, StreamKey};
use crate::source::binance::constants::VENUE_BINANCE_FUTURES;
use crate::source::binance::control::{list_active_collections, stream_key};
use crate::source::binance::message::new_market_data_message;
use crate::source::binance::subscribe::filtered_broadcast_stream;
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
const PRODUCER: &str = "binance_futures_funding";
const WS_URL: &str = "wss://fstream.binance.com/ws/!markPrice@arr@1s";
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const CONNECTION_LIFETIME: Duration = Duration::from_secs(23 * 3600 + 30 * 60);

#[derive(Clone)]
pub struct FundingRateService {
    active: Arc<DashSet<StreamKey>>,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
}

impl FundingRateService {
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
            run_mark_price_stream(tx).await;
        });
    }

    fn key_for(symbol: &str) -> StreamKey {
        stream_key(symbol, VENUE, StreamDataType::Funding)
    }
}

async fn run_mark_price_stream(tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
    loop {
        info!("Binance mark price stream: connecting to {WS_URL}");
        match tokio_tungstenite::connect_async(WS_URL).await {
            Ok((ws_stream, _)) => {
                info!("Binance mark price stream: connected");
                let (_write, mut read) = ws_stream.split();
                let start = std::time::Instant::now();

                while let Some(msg) = read.next().await {
                    if start.elapsed() >= CONNECTION_LIFETIME {
                        info!("Binance mark price stream: scheduled reconnection");
                        break;
                    }
                    match msg {
                        Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                            let items = parse_mark_price_arr(&text);
                            for data in items {
                                let msg = new_market_data_message(VENUE, PRODUCER, data);
                                if tx.send(Ok(msg)).is_err() {
                                    // No subscribers, keep running anyway
                                }
                            }
                        }
                        Ok(tokio_tungstenite::tungstenite::Message::Ping(_)) => {}
                        Err(e) => {
                            error!("Binance mark price stream: WS error: {e}");
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("Binance mark price stream: connect failed: {e}");
            }
        }
        warn!("Binance mark price stream: reconnecting in {RETRY_INTERVAL:?}");
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
}

fn parse_mark_price_arr(text: &str) -> Vec<Data> {
    let arr: Vec<serde_json::Value> = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    arr.iter().filter_map(parse_mark_price_item).collect()
}

fn parse_mark_price_item(v: &serde_json::Value) -> Option<Data> {
    if v.get("e")?.as_str()? != "markPriceUpdate" {
        return None;
    }
    let symbol = v.get("s")?.as_str()?.to_string();
    let timestamp = v.get("E")?.as_i64()?;
    let rate: f64 = v
        .get("r")
        .and_then(|v| v.as_str()?.parse().ok())
        .unwrap_or(0.0);
    let next_funding_time = v.get("T").and_then(|v| v.as_i64()).unwrap_or(0);
    let mark_price: f64 = v
        .get("p")
        .and_then(|v| v.as_str()?.parse().ok())
        .unwrap_or(0.0);
    let index_price: f64 = v
        .get("i")
        .and_then(|v| v.as_str()?.parse().ok())
        .unwrap_or(0.0);

    Some(Data::Funding(FundingRate {
        symbol,
        timestamp,
        rate,
        next_funding_time,
        mark_price,
        index_price,
    }))
}

#[async_trait]
impl Control for FundingRateService {
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
            message: "Stopped all funding collections".to_string(),
        }))
    }
}

#[async_trait]
impl MarketData for FundingRateService {
    type SubscribeStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<MarketDataRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.trim().to_uppercase();
        let stream = filtered_broadcast_stream(
            &self.tx,
            symbol,
            |m, sym, wildcard| matches!(&m.data, Some(Data::Funding(f)) if wildcard || f.symbol == sym),
        );
        Ok(Response::new(stream))
    }
}

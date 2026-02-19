//! Binance Futures open interest poller: REST /fapi/v1/openInterest → OpenInterest.

use crate::config::BinanceRestConfig;
use crate::proto::market_data_message::Data;
use crate::proto::{
    ControlRequest, ControlResponse, ListRequest, ListResponse, MarketDataMessage,
    MarketDataRequest, OpenInterest, StopAllRequest, StopAllResponse,
};
use crate::service::{StreamDataType, StreamKey};
use crate::source::binance::constants::VENUE_BINANCE_FUTURES;
use crate::source::binance::control::{list_active_collections, stream_key};
use crate::source::binance::message::new_market_data_message;
use crate::source::binance::subscribe::filtered_broadcast_stream;
use async_trait::async_trait;
use dashmap::DashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::proto::control_server::Control;
use crate::proto::market_data_server::MarketData;

const VENUE: &str = VENUE_BINANCE_FUTURES;
const PRODUCER: &str = "binance_futures_oi";

#[derive(Clone)]
pub struct OpenInterestService {
    active: Arc<DashSet<StreamKey>>,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
}

impl OpenInterestService {
    pub fn new(cfg: &BinanceRestConfig) -> Self {
        let (tx, _) = broadcast::channel(cfg.channel_capacity);
        let active = Arc::new(DashSet::new());
        let service = Self { active, tx };
        service.spawn_poller(cfg);
        service
    }

    fn spawn_poller(&self, cfg: &BinanceRestConfig) {
        let base_url = cfg.futures_rest_url.clone();
        let symbols = cfg.symbols.clone();
        let interval = Duration::from_secs(cfg.oi_poll_secs);
        let tx = self.tx.clone();
        let client = reqwest::Client::new();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                for sym in &symbols {
                    let url = format!("{base_url}/fapi/v1/openInterest?symbol={sym}");
                    match client.get(&url).send().await {
                        Ok(resp) => match resp.json::<serde_json::Value>().await {
                            Ok(v) => {
                                if let Some(msg) = parse_open_interest(&v) {
                                    let _ =
                                        tx.send(Ok(new_market_data_message(VENUE, PRODUCER, msg)));
                                }
                            }
                            Err(e) => warn!("OI parse error for {sym}: {e}"),
                        },
                        Err(e) => error!("OI fetch error for {sym}: {e}"),
                    }
                }
            }
        });
    }

    fn key_for(symbol: &str) -> StreamKey {
        stream_key(symbol, VENUE, StreamDataType::OpenInterest)
    }
}

fn parse_open_interest(v: &serde_json::Value) -> Option<Data> {
    let symbol = v.get("symbol")?.as_str()?.to_string();
    let open_interest: f64 = v
        .get("openInterest")
        .and_then(|v| v.as_str()?.parse().ok())?;
    let timestamp = v
        .get("time")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    Some(Data::OpenInterest(OpenInterest {
        symbol,
        timestamp,
        open_interest,
        open_interest_value: 0.0,
    }))
}

#[async_trait]
impl Control for OpenInterestService {
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
            message: "Stopped all OI collections".to_string(),
        }))
    }
}

#[async_trait]
impl MarketData for OpenInterestService {
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
            |m, sym, wildcard| matches!(&m.data, Some(Data::OpenInterest(o)) if wildcard || o.symbol == sym),
        );
        Ok(Response::new(stream))
    }
}

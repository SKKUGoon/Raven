//! Binance European Options ticker poller: REST /eapi/v1/ticker → OptionsTicker.
//!
//! Outputs OptionsTicker with venue BINANCE_OPTIONS. Downstream consumers aggregate
//! with Deribit options data for combined positioning intelligence.

use crate::config::BinanceRestConfig;
use crate::proto::market_data_message::Data;
use crate::source::binance::control::{list_active_collections, stream_key};
use crate::source::binance::constants::VENUE_BINANCE_OPTIONS;
use crate::source::binance::message::new_market_data_message;
use crate::source::binance::subscribe::filtered_broadcast_stream;
use crate::proto::{
    ControlRequest, ControlResponse, ListRequest, ListResponse,
    MarketDataMessage, MarketDataRequest, OptionsTicker, StopAllRequest, StopAllResponse,
};
use crate::service::{StreamDataType, StreamKey};
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

const VENUE: &str = VENUE_BINANCE_OPTIONS;
const PRODUCER: &str = "binance_options";

#[derive(Clone)]
pub struct BinanceOptionsService {
    active: Arc<DashSet<StreamKey>>,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
}

impl BinanceOptionsService {
    pub fn new(cfg: &BinanceRestConfig) -> Self {
        let (tx, _) = broadcast::channel(cfg.channel_capacity);
        let active = Arc::new(DashSet::new());
        let service = Self { active, tx };
        service.spawn_poller(cfg);
        service
    }

    fn spawn_poller(&self, cfg: &BinanceRestConfig) {
        let base_url = cfg.options_rest_url.clone();
        let interval = Duration::from_secs(cfg.options_poll_secs);
        let tx = self.tx.clone();
        let client = reqwest::Client::new();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let url = format!("{base_url}/eapi/v1/ticker");
                match client.get(&url).send().await {
                    Ok(resp) => match resp.json::<serde_json::Value>().await {
                        Ok(v) => {
                            let tickers = parse_options_tickers(&v);
                            for data in tickers {
                                let _ = tx.send(Ok(new_market_data_message(VENUE, PRODUCER, data)));
                            }
                        }
                        Err(e) => warn!("Binance options parse error: {e}"),
                    },
                    Err(e) => error!("Binance options fetch error: {e}"),
                }
            }
        });
    }

    fn key_for(symbol: &str) -> StreamKey {
        stream_key(symbol, VENUE, StreamDataType::Ticker)
    }
}

fn parse_options_tickers(v: &serde_json::Value) -> Vec<Data> {
    let arr = match v.as_array() {
        Some(a) => a,
        None => return vec![],
    };
    let now = chrono::Utc::now().timestamp_millis();
    arr.iter()
        .filter_map(|item| {
            let symbol = item.get("symbol")?.as_str()?.to_string();
            // Only BTC options
            if !symbol.starts_with("BTC") {
                return None;
            }
            let open_interest = item
                .get("openInterest")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);
            let mark_iv = item
                .get("markIv")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);
            let best_bid_price = item
                .get("bidPrice")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);
            let best_ask_price = item
                .get("askPrice")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);
            let mark_price = item
                .get("markPrice")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);
            let underlying_price = item
                .get("exercisePrice")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);
            let last_price = item
                .get("lastPrice")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);
            let bid_iv = item
                .get("bidIv")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);
            let ask_iv = item
                .get("askIv")
                .and_then(|v| v.as_str()?.parse().ok())
                .unwrap_or(0.0);

            Some(Data::OptionsTicker(OptionsTicker {
                symbol,
                timestamp: now,
                open_interest,
                mark_iv,
                best_bid_price,
                best_ask_price,
                mark_price,
                underlying_price,
                last_price,
                bid_iv,
                ask_iv,
            }))
        })
        .collect()
}

#[async_trait]
impl Control for BinanceOptionsService {
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
            message: "Stopped all Binance options collections".to_string(),
        }))
    }
}

#[async_trait]
impl MarketData for BinanceOptionsService {
    type SubscribeStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<MarketDataRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let symbol = req.symbol.trim().to_uppercase();
        let stream = filtered_broadcast_stream(&self.tx, symbol, |m, sym, wildcard| {
            matches!(&m.data, Some(Data::OptionsTicker(t)) if wildcard || t.symbol == sym)
        });
        Ok(Response::new(stream))
    }
}

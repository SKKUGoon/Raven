//! Generic Deribit single-channel service: one WebSocket, one data type → Control + MarketData.
//!
//! Instantiated three times as separate microservices:
//! - `DeribitTickerService`  (ticker.BTC-OPTION.100ms → OptionsTicker)
//! - `DeribitTradesService`  (trades.BTC-OPTION.100ms → Trade)
//! - `DeribitIndexService`   (deribit_price_index.btc_usd → PriceIndex)

use crate::proto::market_data_message::Data;
use crate::proto::{
    CollectionInfo, ControlRequest, ControlResponse, ListRequest, ListResponse, MarketDataMessage,
    MarketDataRequest, StopAllRequest, StopAllResponse,
};
use crate::service::{StreamDataType, StreamKey};
use crate::source::deribit::client::{self, OnNotification};
use crate::source::deribit::constants::VENUE_DERIBIT;
use crate::source::deribit::parsing;
use async_trait::async_trait;
use dashmap::DashSet;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::info;

use crate::proto::control_server::Control;
use crate::proto::market_data_server::MarketData;

const VENUE: &str = VENUE_DERIBIT;

// ---------------------------------------------------------------------------
// Generic service
// ---------------------------------------------------------------------------

/// A Deribit microservice that subscribes to a single channel and produces one data type.
#[derive(Clone)]
pub struct DeribitService {
    name: &'static str,
    data_type: StreamDataType,
    active: Arc<DashSet<StreamKey>>,
    tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
}

impl DeribitService {
    fn new(
        name: &'static str,
        ws_url: String,
        channel: String,
        data_type: StreamDataType,
        channel_capacity: usize,
        on_notification: OnNotification,
    ) -> Self {
        let (tx, _) = broadcast::channel(channel_capacity);
        let active = Arc::new(DashSet::new());
        let service = Self {
            name,
            data_type,
            active,
            tx,
        };
        service.spawn_connection(ws_url, channel, on_notification);
        service
    }

    fn spawn_connection(&self, ws_url: String, channel: String, on_notification: OnNotification) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            client::run(ws_url, vec![channel], on_notification, tx).await;
        });
    }

    fn key_for(&self, symbol: &str) -> StreamKey {
        StreamKey {
            symbol: symbol.trim().to_uppercase(),
            venue: Some(VENUE.to_string()),
            data_type: self.data_type,
        }
    }

    fn is_active(&self, key: &StreamKey) -> bool {
        self.active.contains(key)
    }

    fn active_keys(&self) -> Vec<StreamKey> {
        self.active.iter().map(|r| r.key().clone()).collect()
    }
}

// ---------------------------------------------------------------------------
// Control impl (shared by all three)
// ---------------------------------------------------------------------------

#[async_trait]
impl Control for DeribitService {
    async fn start_collection(
        &self,
        request: Request<ControlRequest>,
    ) -> Result<Response<ControlResponse>, Status> {
        let req = request.into_inner();
        let venue = req.venue.trim().to_uppercase();
        if !venue.is_empty() && venue != VENUE {
            return Err(Status::invalid_argument(format!(
                "Invalid venue '{venue}'; expected {VENUE}"
            )));
        }
        let incoming_dt = StreamDataType::from_proto_i32(req.data_type);
        if incoming_dt != self.data_type {
            return Err(Status::invalid_argument(format!(
                "{} only supports data_type {:?}",
                self.name, self.data_type
            )));
        }
        let symbol = normalise_symbol(&req.symbol, self.data_type);
        if symbol.is_empty() {
            return Err(Status::invalid_argument("symbol is empty"));
        }
        let key = self.key_for(&symbol);
        self.active.insert(key.clone());
        info!("{}: started collection for {key}", self.name);
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
        let symbol = normalise_symbol(&req.symbol, self.data_type);
        let key = self.key_for(&symbol);
        self.active.remove(&key);
        info!("{}: stopped collection for {key}", self.name);
        Ok(Response::new(ControlResponse {
            success: true,
            message: format!("Stopped collection for {key}"),
        }))
    }

    async fn list_collections(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let collections = self
            .active_keys()
            .into_iter()
            .map(|k| CollectionInfo {
                symbol: k.to_string(),
                status: "active".to_string(),
                subscriber_count: 0,
            })
            .collect();
        Ok(Response::new(ListResponse { collections }))
    }

    async fn stop_all_collections(
        &self,
        _request: Request<StopAllRequest>,
    ) -> Result<Response<StopAllResponse>, Status> {
        self.active.clear();
        info!("{}: stopped all collections", self.name);
        Ok(Response::new(StopAllResponse {
            success: true,
            message: format!("Stopped all {} collections", self.name),
        }))
    }
}

// ---------------------------------------------------------------------------
// MarketData impl (shared by all three, filters by symbol)
// ---------------------------------------------------------------------------

#[async_trait]
impl MarketData for DeribitService {
    type SubscribeStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<MarketDataRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let venue = req.venue.trim().to_uppercase();
        if !venue.is_empty() && venue != VENUE {
            return Err(Status::invalid_argument(format!(
                "Invalid venue '{venue}'; expected {VENUE}"
            )));
        }
        let symbol = req.symbol.trim().to_uppercase();
        let wildcard = symbol.is_empty() || symbol == "*";
        if !wildcard {
            let key = self.key_for(&symbol);
            if !self.is_active(&key) {
                return Err(Status::failed_precondition(format!(
                    "Stream not started for {key}. Call Control.StartCollection first."
                )));
            }
        }

        let rx = self.tx.subscribe();
        let dt = self.data_type;
        let stream = tokio_stream::StreamExt::filter_map(BroadcastStream::new(rx), move |item| {
            let sym = symbol.clone();
            match item {
                Ok(Ok(m)) => {
                    let pass = match (&m.data, dt) {
                        (Some(Data::OptionsTicker(t)), StreamDataType::Ticker) => {
                            wildcard || t.symbol == sym
                        }
                        (Some(Data::Trade(t)), StreamDataType::Trade) => {
                            wildcard || t.symbol == sym
                        }
                        (Some(Data::PriceIndex(p)), StreamDataType::PriceIndex) => {
                            wildcard || sym == "BTC_USD" || p.index_name == sym.to_lowercase()
                        }
                        _ => false,
                    };
                    if pass {
                        Some(Ok(m))
                    } else {
                        None
                    }
                }
                Ok(Err(e)) => Some(Err(e)),
                Err(_) => Some(Err(Status::internal("Stream lagged"))),
            }
        });
        Ok(Response::new(Box::pin(stream)))
    }
}

// ---------------------------------------------------------------------------
// Constructors — one per microservice
// ---------------------------------------------------------------------------

/// Options **ticker** service (open interest, IV, mark price, best bid/ask).
pub fn new_ticker_service(ws_url: String, channel_capacity: usize) -> DeribitService {
    let handler: OnNotification = Box::new(|channel, data_str| {
        let data: serde_json::Value = match serde_json::from_str(data_str) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        parsing::parse_ticker(channel, &data).into_iter().collect()
    });
    DeribitService::new(
        "DeribitTicker",
        ws_url,
        client::CHANNEL_TICKER.to_string(),
        StreamDataType::Ticker,
        channel_capacity,
        handler,
    )
}

/// Options **trades** service (every trade execution).
pub fn new_trades_service(ws_url: String, channel_capacity: usize) -> DeribitService {
    let handler: OnNotification = Box::new(|channel, data_str| {
        let data: serde_json::Value = match serde_json::from_str(data_str) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        parsing::parse_trades(channel, &data)
    });
    DeribitService::new(
        "DeribitTrades",
        ws_url,
        client::CHANNEL_TRADES.to_string(),
        StreamDataType::Trade,
        channel_capacity,
        handler,
    )
}

/// Underlying **price index** service (btc_usd).
pub fn new_index_service(ws_url: String, channel_capacity: usize) -> DeribitService {
    let handler: OnNotification = Box::new(|channel, data_str| {
        let data: serde_json::Value = match serde_json::from_str(data_str) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        parsing::parse_price_index(channel, &data)
            .into_iter()
            .collect()
    });
    DeribitService::new(
        "DeribitIndex",
        ws_url,
        client::CHANNEL_PRICE_INDEX.to_string(),
        StreamDataType::PriceIndex,
        channel_capacity,
        handler,
    )
}

fn normalise_symbol(symbol: &str, dt: StreamDataType) -> String {
    let s = symbol.trim().to_uppercase();
    if dt == StreamDataType::PriceIndex && s.is_empty() {
        "BTC_USD".to_string()
    } else {
        s
    }
}

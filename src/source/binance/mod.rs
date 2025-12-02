use crate::proto::market_data_message;
use crate::proto::{MarketDataMessage, Trade};
use futures_util::StreamExt;
use prometheus::{IntCounterVec, IntGauge};
use tokio::sync::broadcast;
use tonic::Status;
use tracing::{error, info};
use url::Url;

pub mod future;
pub mod spot;

#[derive(Clone)]
pub struct BinanceWsClient {
    base_url: String,
    stream_type: String,
    exchange_name: String,
    parser: fn(&str, &str) -> Option<Trade>,
}

impl BinanceWsClient {
    pub fn new(
        base_url: String,
        stream_type: String,
        exchange_name: String,
        parser: fn(&str, &str) -> Option<Trade>,
    ) -> Self {
        Self {
            base_url,
            stream_type,
            exchange_name,
            parser,
        }
    }

    pub async fn run(
        &self,
        symbol: String,
        tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
        metrics_processed: &IntCounterVec,
        metrics_active: &IntGauge,
    ) {
        let stream_name = format!("{}@{}", symbol.to_lowercase(), self.stream_type);
        let url_str = format!("{}{}", self.base_url, stream_name);
        // Ensure the URL is valid, handle error gracefully if needed, but panic is okay if config is hardcoded.
        let url = match Url::parse(&url_str) {
            Ok(u) => u,
            Err(e) => {
                error!("Invalid URL {}: {}", url_str, e);
                return;
            }
        };

        info!("Connecting to {} WS: {}", self.exchange_name, url);

        match tokio_tungstenite::connect_async(url.to_string()).await {
            Ok((ws_stream, _)) => {
                info!("Connected to {} for {}", self.exchange_name, symbol);
                metrics_active.inc();
                let (_, mut read) = ws_stream.split();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                            if let Some(trade) = (self.parser)(&text, &symbol) {
                                metrics_processed.with_label_values(&[&symbol]).inc();
                                let msg = MarketDataMessage {
                                    exchange: self.exchange_name.clone(),
                                    data: Some(market_data_message::Data::Trade(trade)),
                                };
                                if tx.send(Ok(msg)).is_err() {
                                    break; // No subscribers
                                }
                            }
                        }
                        Ok(tokio_tungstenite::tungstenite::Message::Ping(_)) => {
                            // Auto-pong handled by tungstenite
                        }
                        Err(e) => {
                            error!("WS error for {}: {}", symbol, e);
                            break;
                        }
                        _ => {}
                    }
                }
                metrics_active.dec();
            }
            Err(e) => {
                error!(
                    "Failed to connect to {} for {}: {}",
                    self.exchange_name, symbol, e
                );
            }
        }

        info!("WS task ended for {}", symbol);
    }
}

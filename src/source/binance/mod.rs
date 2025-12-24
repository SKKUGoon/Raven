use crate::proto::market_data_message;
use crate::proto::MarketDataMessage;
use futures_util::StreamExt;
use prometheus::{IntCounterVec, IntGauge};
use tokio::sync::broadcast;
use tonic::Status;
use tracing::{error, info, warn};
use url::Url;

pub mod future;
pub mod spot;

#[derive(Clone)]
pub struct BinanceWsClient {
    base_url: String,
    exchange_name: String,
    venue: String,
}

impl BinanceWsClient {
    pub fn new(base_url: String, exchange_name: String, venue: String) -> Self {
        Self {
            base_url,
            exchange_name,
            venue,
        }
    }

    pub async fn run<F>(
        &self,
        symbol: String,
        stream_type: String,
        parser: F,
        tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
        metrics_processed: &IntCounterVec,
        metrics_active: &IntGauge,
    ) where
        F: Fn(&str, &str) -> Option<market_data_message::Data> + Send + Sync + 'static,
    {
        let stream_name = format!("{}@{}", symbol.to_lowercase(), stream_type);
        let url_str = format!("{}{}", self.base_url, stream_name);
        // Ensure the URL is valid, handle error gracefully if needed, but panic is okay if config is hardcoded.
        let url = match Url::parse(&url_str) {
            Ok(u) => u,
            Err(e) => {
                error!("Invalid URL {}: {}", url_str, e);
                return;
            }
        };

        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 10;
        const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
        // 23 hours and 30 minutes
        const CONNECTION_LIFETIME: std::time::Duration = std::time::Duration::from_secs(23 * 3600 + 30 * 60);

        loop {
            info!("Connecting to {} WS: {}", self.exchange_name, url);

            match tokio_tungstenite::connect_async(url.to_string()).await {
                Ok((ws_stream, _)) => {
                    info!("Connected to {} for {}", self.exchange_name, symbol);
                    retry_count = 0; // Reset on successful connection
                    metrics_active.inc();
                    let (_, mut read) = ws_stream.split();

                    // Use a flag to determine if we should break the outer loop (e.g. no subscribers)
                    // or continue (reconnect)
                    let should_reconnect = {
                        let process_stream = async {
                            while let Some(msg) = read.next().await {
                                match msg {
                                    Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                        if let Some(data) = parser(&text, &symbol) {
                                            metrics_processed.with_label_values(&[&symbol]).inc();
                                            let msg = MarketDataMessage {
                                                // Backwards-compat: keep exchange as producer for older consumers.
                                                exchange: self.exchange_name.clone(),
                                                venue: self.venue.clone(),
                                                producer: self.exchange_name.clone(),
                                                data: Some(data),
                                            };
                                            if tx.send(Ok(msg)).is_err() {
                                                return false; // No subscribers, stop completely
                                            }
                                        }
                                    }
                                    Ok(tokio_tungstenite::tungstenite::Message::Ping(_)) => {
                                        // Auto-pong handled by tungstenite
                                    }
                                    Err(e) => {
                                        error!("WS error for {}: {}", symbol, e);
                                        return true; // Error, try to reconnect
                                    }
                                    _ => {}
                                }
                            }
                            true // Stream ended normally (eof), try to reconnect
                        };

                        tokio::select! {
                            res = process_stream => res,
                            _ = tokio::time::sleep(CONNECTION_LIFETIME) => {
                                info!("Scheduled reconnection for {} after {:?}", symbol, CONNECTION_LIFETIME);
                                true // Time to reconnect
                            }
                        }
                    };

                    metrics_active.dec();

                    if !should_reconnect {
                        break;
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to connect to {} for {}: {}",
                        self.exchange_name, symbol, e
                    );
                }
            }

            if retry_count >= MAX_RETRIES {
                error!("Max retries ({}) reached for {}. Exiting.", MAX_RETRIES, symbol);
                break;
            }

            retry_count += 1;
            warn!(
                "Disconnected/Failed. Retrying in {:?} (attempt {}/{}) for {}",
                RETRY_INTERVAL, retry_count, MAX_RETRIES, symbol
            );
            tokio::time::sleep(RETRY_INTERVAL).await;
        }

        info!("WS task ended for {}", symbol);
    }
}

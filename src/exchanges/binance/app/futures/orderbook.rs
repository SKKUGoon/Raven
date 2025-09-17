use crate::app::shutdown::DataCollector;
use crate::error::RavenResult;
use crate::exchanges::binance::BinanceFuturesParser;
use crate::exchanges::types::{
    DataType, Exchange, MarketData, MarketDataMessage, SubscriptionRequest,
};
use crate::exchanges::websocket::{ExchangeWebSocketClient, WebSocketParser};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

#[derive(Clone)]
pub struct BinanceFuturesOrderbook {
    current_data: Arc<RwLock<Option<MarketDataMessage>>>,
    symbol: String,
    ws_client: Option<Arc<RwLock<ExchangeWebSocketClient>>>,
}

impl BinanceFuturesOrderbook {
    pub fn new(symbol: String) -> RavenResult<Self> {
        let parser = Box::new(BinanceFuturesParser::new()) as Box<dyn WebSocketParser>;
        let mut ws_client = ExchangeWebSocketClient::new(parser)?;

        let subscription = SubscriptionRequest {
            exchange: Exchange::BinanceFutures,
            symbol: symbol.clone(),
            data_type: DataType::OrderBook,
        };
        ws_client.add_subscription(subscription);

        Ok(Self {
            current_data: Arc::new(RwLock::new(None)),
            symbol,
            ws_client: Some(Arc::new(RwLock::new(ws_client))),
        })
    }

    pub async fn start_streaming(
        &mut self,
    ) -> RavenResult<mpsc::UnboundedReceiver<MarketDataMessage>> {
        let (tx, rx) = mpsc::unbounded_channel();

        if let Some(ws_client) = self.ws_client.take() {
            let current_data = Arc::clone(&self.current_data);
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                let (msg_tx, mut msg_rx) = mpsc::unbounded_channel();

                // Start WebSocket client
                tokio::spawn(async move {
                    let mut client = ws_client.write().await;
                    if let Err(e) = client.connect_and_stream(msg_tx).await {
                        eprintln!("WebSocket error: {e}");
                    }
                });

                // Handle incoming messages
                while let Some(message) = msg_rx.recv().await {
                    // Update internal state
                    {
                        let mut current = current_data.write().await;
                        *current = Some(message.clone());
                    }

                    // Forward to external subscribers
                    if tx_clone.send(message).is_err() {
                        break;
                    }
                }
            });
        }

        Ok(rx)
    }

    pub async fn get_current_data(&self) -> Option<MarketDataMessage> {
        self.current_data.read().await.clone()
    }

    pub fn get_symbol(&self) -> &str {
        &self.symbol
    }

    pub fn get_exchange(&self) -> Exchange {
        Exchange::BinanceFutures
    }

    pub async fn get_best_bid(&self) -> Option<(f64, f64)> {
        if let Some(data) = self.get_current_data().await {
            if let MarketData::OrderBook { bids, .. } = data.data {
                bids.first().copied()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn get_best_ask(&self) -> Option<(f64, f64)> {
        if let Some(data) = self.get_current_data().await {
            if let MarketData::OrderBook { asks, .. } = data.data {
                asks.first().copied()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn get_spread(&self) -> Option<f64> {
        let best_bid = self.get_best_bid().await?;
        let best_ask = self.get_best_ask().await?;
        Some(best_ask.0 - best_bid.0)
    }

    pub async fn get_mid_price(&self) -> Option<f64> {
        let best_bid = self.get_best_bid().await?;
        let best_ask = self.get_best_ask().await?;
        Some((best_bid.0 + best_ask.0) / 2.0)
    }
}

pub async fn initialize_binance_futures_orderbook(
    symbol: String,
) -> RavenResult<(
    Arc<BinanceFuturesOrderbook>,
    mpsc::UnboundedReceiver<MarketDataMessage>,
)> {
    info!("Send out the raven for Binance Futures Orderbook for symbol: {symbol}");

    let mut binance_collector = BinanceFuturesOrderbook::new(symbol)?;
    let rx = binance_collector.start_streaming().await?;

    Ok((Arc::new(binance_collector), rx))
}

impl DataCollector for BinanceFuturesOrderbook {
    fn name(&self) -> &'static str {
        "BinanceFuturesOrderbook"
    }
}

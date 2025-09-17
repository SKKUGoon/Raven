use crate::error::RavenResult;
use crate::exchanges::binance::BinanceSpotParser;
use crate::exchanges::types::{
    DataType, Exchange, MarketData, MarketDataMessage, SubscriptionRequest, TradeSide,
};
use crate::exchanges::websocket::{ExchangeWebSocketClient, WebSocketParser};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[derive(Clone)]
pub struct BinanceSpotTrade {
    current_data: Arc<RwLock<Option<MarketDataMessage>>>,
    symbol: String,
    ws_client: Option<Arc<RwLock<ExchangeWebSocketClient>>>,
}

impl BinanceSpotTrade {
    pub fn new(symbol: String) -> RavenResult<Self> {
        let parser = Box::new(BinanceSpotParser::new()) as Box<dyn WebSocketParser>;
        let mut ws_client = ExchangeWebSocketClient::new(parser)?;

        let subscription = SubscriptionRequest {
            exchange: Exchange::BinanceSpot,
            symbol: symbol.clone(),
            data_type: DataType::SpotTrade,
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
        Exchange::BinanceSpot
    }

    pub async fn get_last_price(&self) -> Option<f64> {
        if let Some(data) = self.get_current_data().await {
            if let MarketData::SpotTrade { price, .. } = data.data {
                Some(price)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn get_last_size(&self) -> Option<f64> {
        if let Some(data) = self.get_current_data().await {
            if let MarketData::SpotTrade { size, .. } = data.data {
                Some(size)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn get_last_side(&self) -> Option<TradeSide> {
        if let Some(data) = self.get_current_data().await {
            if let MarketData::SpotTrade { side, .. } = data.data {
                Some(side)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn get_last_trade_id(&self) -> Option<String> {
        if let Some(data) = self.get_current_data().await {
            if let MarketData::SpotTrade { trade_id, .. } = data.data {
                Some(trade_id)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn get_trade_value(&self) -> Option<f64> {
        let price = self.get_last_price().await?;
        let size = self.get_last_size().await?;
        Some(price * size)
    }

    pub async fn is_buyer_maker(&self) -> Option<bool> {
        if let Some(side) = self.get_last_side().await {
            match side {
                TradeSide::Buy => Some(false),
                TradeSide::Sell => Some(true),
            }
        } else {
            None
        }
    }
}

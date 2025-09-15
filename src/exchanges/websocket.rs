use crate::error::{RavenError, RavenResult};
use crate::exchanges::types::*;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use url::Url;

#[async_trait]
pub trait WebSocketParser: Send + Sync {
    fn exchange(&self) -> Exchange;

    fn create_subscription_message(
        &self,
        subscription: &SubscriptionRequest,
    ) -> RavenResult<String>;

    fn parse_orderbook(&self, message: &str) -> RavenResult<Option<MarketDataMessage>>;
    fn parse_candle(&self, message: &str) -> RavenResult<Option<MarketDataMessage>>;
    fn parse_spot_trade(&self, message: &str) -> RavenResult<Option<MarketDataMessage>>;
    fn parse_future_trade(&self, message: &str) -> RavenResult<Option<MarketDataMessage>>;
    fn parse_ticker(&self, message: &str) -> RavenResult<Option<MarketDataMessage>>;

    fn parse_message(
        &self,
        message: &str,
        data_type: &DataType,
    ) -> RavenResult<Option<MarketDataMessage>> {
        match data_type {
            DataType::OrderBook => self.parse_orderbook(message),
            DataType::Candle => self.parse_candle(message),
            DataType::SpotTrade => self.parse_spot_trade(message),
            DataType::FutureTrade => self.parse_future_trade(message),
            DataType::Ticker => self.parse_ticker(message),
        }
    }
}

pub struct ExchangeWebSocketClient {
    parser: Box<dyn WebSocketParser>,
    url: Url,
    subscriptions: Vec<SubscriptionRequest>,
    connection_start: Option<Instant>,
    max_connection_duration: Duration,
}

impl ExchangeWebSocketClient {
    pub fn new(parser: Box<dyn WebSocketParser>) -> RavenResult<Self> {
        let exchange = parser.exchange();
        let url = Url::parse(exchange.websocket_url())
            .map_err(|e| RavenError::config_error(format!("Invalid WebSocket URL: {e}")))?;

        Ok(Self {
            parser,
            url,
            subscriptions: Vec::new(),
            connection_start: None,
            max_connection_duration: Duration::from_secs(23 * 3600), // 23 hours to be safe
        })
    }

    pub fn add_subscription(&mut self, subscription: SubscriptionRequest) {
        self.subscriptions.push(subscription);
    }

    pub async fn connect_and_stream(
        &mut self,
        message_sender: tokio::sync::mpsc::UnboundedSender<MarketDataMessage>,
    ) -> RavenResult<()> {
        loop {
            self.connection_start = Some(Instant::now());

            match self.connect_with_retry(&message_sender).await {
                Ok(_) => {
                    warn!(
                        "WebSocket connection closed for {}, reconnecting...",
                        self.parser.exchange()
                    );
                }
                Err(e) => {
                    error!("WebSocket error for {}: {}", self.parser.exchange(), e);
                }
            }

            sleep(Duration::from_secs(5)).await;
        }
    }

    async fn connect_with_retry(
        &self,
        message_sender: &tokio::sync::mpsc::UnboundedSender<MarketDataMessage>,
    ) -> RavenResult<()> {
        info!(
            "Connecting to {} WebSocket: {}",
            self.parser.exchange(),
            self.url
        );

        let (ws_stream, _) = connect_async(self.url.as_str()).await.map_err(|e| {
            RavenError::connection_error(format!("WebSocket connection failed: {e}"))
        })?;

        info!("Connected to {} WebSocket", self.parser.exchange());

        let (mut write, mut read) = ws_stream.split();

        // Send subscriptions
        for subscription in &self.subscriptions {
            let subscribe_msg = self.parser.create_subscription_message(subscription)?;
            write
                .send(Message::Text(subscribe_msg.into()))
                .await
                .map_err(|e| {
                    RavenError::connection_error(format!("Failed to send subscription: {e}"))
                })?;

            debug!(
                "Subscribed to {:?} {} on {}",
                subscription.data_type,
                subscription.symbol,
                self.parser.exchange()
            );
        }

        let mut ping_interval = interval(Duration::from_secs(30));
        let mut reconnect_check = interval(Duration::from_secs(3600)); // Check every hour

        loop {
            // Check if we need to proactively reconnect (before 24h limit)
            if let Some(start_time) = self.connection_start {
                if start_time.elapsed() >= self.max_connection_duration {
                    info!(
                        "Proactively reconnecting {} WebSocket before 24h limit",
                        self.parser.exchange()
                    );
                    break;
                }
            }

            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_message(&text, message_sender).await {
                                warn!("Failed to handle message: {}", e);
                            }
                        }
                        Some(Ok(Message::Ping(payload))) => {
                            if let Err(e) = write.send(Message::Pong(payload)).await {
                                error!("Failed to send pong: {}", e);
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("WebSocket closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            warn!("WebSocket stream ended");
                            break;
                        }
                        _ => {}
                    }
                }
                _ = ping_interval.tick() => {
                    if let Err(e) = write.send(Message::Ping(vec![].into())).await {
                        error!("Failed to send ping: {}", e);
                        break;
                    }
                }
                _ = reconnect_check.tick() => {
                    // Just continue the loop, the time check at the top will handle reconnection
                }
            }
        }

        Ok(())
    }

    async fn handle_message(
        &self,
        message: &str,
        sender: &tokio::sync::mpsc::UnboundedSender<MarketDataMessage>,
    ) -> RavenResult<()> {
        // Try to parse with each subscription type until we find a match
        for subscription in &self.subscriptions {
            if let Some(market_data) = self
                .parser
                .parse_message(message, &subscription.data_type)?
            {
                sender.send(market_data).map_err(|_| {
                    RavenError::internal_error("Failed to send message".to_string())
                })?;
                return Ok(());
            }
        }

        // If no subscription matched, it might be a control message - just debug log it
        debug!(
            "Unhandled message from {}: {}",
            self.parser.exchange(),
            message
        );
        Ok(())
    }
}

use crate::common::error::RavenResult;
use crate::server::exchanges::types::*;
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

    fn create_unsubscribe_message(&self, subscription: &SubscriptionRequest)
        -> RavenResult<String>;

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

enum ConnectionOutcome {
    Reconnect,
    Terminate,
}

impl ExchangeWebSocketClient {
    pub fn new(parser: Box<dyn WebSocketParser>) -> RavenResult<Self> {
        let exchange = parser.exchange();
        let url = Url::parse(exchange.websocket_url()).map_err(|e| {
            crate::raven_error!(config_error, format!("Invalid WebSocket URL: {e}"))
        })?;

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
                Ok(ConnectionOutcome::Reconnect) => {
                    warn!(
                        "WebSocket connection closed for {}, reconnecting...",
                        self.parser.exchange()
                    );
                }
                Ok(ConnectionOutcome::Terminate) => {
                    info!(
                        "WebSocket connection terminated for {}",
                        self.parser.exchange()
                    );
                    return Ok(());
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
    ) -> RavenResult<ConnectionOutcome> {
        info!(
            "Connecting to {} WebSocket: {}",
            self.parser.exchange(),
            self.url
        );

        let (ws_stream, _) = connect_async(self.url.as_str()).await.map_err(|e| {
            crate::raven_error!(
                connection_error,
                format!("WebSocket connection failed: {e}")
            )
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
                    crate::raven_error!(
                        connection_error,
                        format!("Failed to send subscription: {e}")
                    )
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
        let sender_closed = message_sender.closed();
        tokio::pin!(sender_closed);

        let mut terminate_requested = false;

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
                _ = &mut sender_closed => {
                    info!(
                        "Output channel closed for {}, sending unsubscribe",
                        self.parser.exchange()
                    );
                    for subscription in &self.subscriptions {
                        if let Ok(unsubscribe_msg) = self.parser.create_unsubscribe_message(subscription) {
                            if let Err(e) = write.send(Message::Text(unsubscribe_msg.into())).await {
                                warn!(
                                    "Failed to send unsubscribe for {} {}: {e}",
                                    self.parser.exchange(),
                                    subscription.symbol
                                );
                            }
                        }
                    }
                    terminate_requested = true;
                    break;
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            match self.handle_message(&text, message_sender).await {
                                Ok(channel_closed) => {
                                    if channel_closed {
                                        info!(
                                            "Delivery channel closed for {}, unsubscribing",
                                            self.parser.exchange()
                                        );
                                        for subscription in &self.subscriptions {
                                            if let Ok(unsubscribe_msg) = self.parser.create_unsubscribe_message(subscription) {
                                                if let Err(e) = write.send(Message::Text(unsubscribe_msg.into())).await {
                                                    warn!(
                                                        "Failed to send unsubscribe for {} {}: {e}",
                                                        self.parser.exchange(),
                                                        subscription.symbol
                                                    );
                                                }
                                            }
                                        }
                                        terminate_requested = true;
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to handle message: {e}. Message: {text}");
                                }
                            }
                        }
                        Some(Ok(Message::Ping(payload))) => {
                            if let Err(e) = write.send(Message::Pong(payload)).await {
                                error!("Failed to send pong: {e}");
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("WebSocket closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {e}");
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
                        error!("Failed to send ping: {e}");
                        break;
                    }
                }
                _ = reconnect_check.tick() => {
                    // Just continue the loop, the time check at the top will handle reconnection
                }
            }
        }

        if terminate_requested {
            Ok(ConnectionOutcome::Terminate)
        } else {
            Ok(ConnectionOutcome::Reconnect)
        }
    }

    async fn handle_message(
        &self,
        message: &str,
        sender: &tokio::sync::mpsc::UnboundedSender<MarketDataMessage>,
    ) -> RavenResult<bool> {
        let mut handled_any = false;

        for subscription in &self.subscriptions {
            if let Some(market_data) = self
                .parser
                .parse_message(message, &subscription.data_type)?
            {
                handled_any = true;
                if sender.send(market_data).is_err() {
                    return Ok(true);
                }
            }
        }

        // If no subscription matched, it might be a control message - just debug log it
        if !handled_any {
            debug!(
                "Unhandled message from {}: {}",
                self.parser.exchange(),
                message
            );
        }

        Ok(false)
    }
}

//! Deribit WebSocket client: JSON-RPC 2.0 connect, subscribe, read loop.

use crate::proto::market_data_message;
use crate::proto::MarketDataMessage;
use crate::source::deribit::constants::{PRODUCER_DERIBIT, VENUE_DERIBIT};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use serde_json::json;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;
use tonic::Status;
use tracing::{error, info};

const SUBSCRIBE_ID: u64 = 1;
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const CONNECTION_LIFETIME: Duration = Duration::from_secs(23 * 3600 + 30 * 60);

/// All three Deribit channels for BTC options intelligence.
pub const CHANNEL_TICKER: &str = "ticker.BTC-OPTION.100ms";
pub const CHANNEL_TRADES: &str = "trades.BTC-OPTION.100ms";
pub const CHANNEL_PRICE_INDEX: &str = "deribit_price_index.btc_usd";

/// Callback: (channel, data_json) -> zero or more data payloads to broadcast.
pub type OnNotification = Box<
    dyn Fn(&str, &str) -> Vec<market_data_message::Data> + Send + Sync,
>;

/// Run the Deribit WebSocket connection: connect, subscribe to `channels`, then read loop.
/// Calls `on_notification` for each subscription notification; messages are broadcast by the service.
pub async fn run(
    ws_url: String,
    channels: Vec<String>,
    on_notification: OnNotification,
    tx: tokio::sync::broadcast::Sender<Result<MarketDataMessage, Status>>,
) {
    let on_notification = Arc::new(on_notification);
    let mut request_id = SUBSCRIBE_ID;
    loop {
        info!("Deribit: connecting to {}", ws_url);
        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                info!("Deribit: connected");
                let (mut write, mut read) = ws_stream.split();

                let subscribe_msg = serde_json::to_string(&json!({
                    "jsonrpc": "2.0",
                    "method": "public/subscribe",
                    "params": {
                        "channels": channels
                    },
                    "id": request_id
                }))
                .expect("subscribe JSON");
                if let Err(e) = write.send(Message::Text(subscribe_msg.into())).await {
                    error!("Deribit: failed to send subscribe: {}", e);
                    tokio::time::sleep(RETRY_INTERVAL).await;
                    continue;
                }
                request_id = request_id.wrapping_add(1);

                let start = std::time::Instant::now();
                let mut should_reconnect = false;

                while let Some(msg) = read.next().await {
                    if start.elapsed() >= CONNECTION_LIFETIME {
                        info!("Deribit: scheduled reconnection after {:?}", CONNECTION_LIFETIME);
                        should_reconnect = true;
                        break;
                    }
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Some((channel, data_str)) = parse_subscription_notification(&text)
                            {
                                for data in on_notification(channel.as_str(), data_str.as_str()) {
                                    let msg = MarketDataMessage {
                                        exchange: String::new(),
                                        venue: VENUE_DERIBIT.to_string(),
                                        producer: PRODUCER_DERIBIT.to_string(),
                                        data: Some(data),
                                    };
                                    if tx.send(Ok(msg)).is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                        Ok(Message::Ping(_)) => {}
                        Err(e) => {
                            error!("Deribit: WS error: {}", e);
                            should_reconnect = true;
                            break;
                        }
                        _ => {}
                    }
                }
                if !should_reconnect {
                    return;
                }
            }
            Err(e) => {
                error!("Deribit: connect failed: {}", e);
            }
        }
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
}

/// Parse JSON-RPC notification: method "subscription", params.channel and params.data (as string).
fn parse_subscription_notification(text: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("method")?.as_str()? != "subscription" {
        return None;
    }
    let params = v.get("params")?;
    let channel = params.get("channel")?.as_str()?.to_string();
    let data = params.get("data")?;
    let data_str = data.to_string();
    Some((channel, data_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::time::timeout;

    /// Integration test: connect to Deribit testnet, subscribe, and assert we receive
    /// at least one subscription notification. Run with `cargo test deribit_feed_subscription -- --ignored`.
    #[tokio::test]
    #[ignore] // hits real Deribit API; run with: cargo test deribit_feed_subscription -- --ignored
    async fn deribit_feed_subscription() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();
        const WS_URL: &str = "wss://test.deribit.com/ws/api/v2";
        let (ws_stream, _) = tokio_tungstenite::connect_async(WS_URL)
            .await
            .expect("connect");
        let (mut write, mut read) = ws_stream.split();

        let subscribe_msg = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "method": "public/subscribe",
            "params": { "channels": [CHANNEL_TICKER, CHANNEL_TRADES, CHANNEL_PRICE_INDEX] },
            "id": 1
        }))
        .unwrap();
        write
            .send(Message::Text(subscribe_msg.into()))
            .await
            .expect("send subscribe");

        let mut got_notification = false;
        let deadline = std::time::Duration::from_secs(20);
        while let Some(msg) = timeout(deadline, read.next()).await.ok().flatten() {
            let Ok(Message::Text(text)) = msg else { continue };
            let v: serde_json::Value = match serde_json::from_str(&text) {
                Ok(x) => x,
                Err(_) => continue,
            };
            if v.get("method").and_then(|m| m.as_str()) == Some("subscription") {
                got_notification = true;
                break;
            }
        }
        assert!(
            got_notification,
            "expected at least one subscription notification from Deribit within {deadline:?}"
        );
    }

    /// Run the feed for 5 seconds and print every incoming subscription message.
    /// Run with: cargo test deribit_feed_print_5_sec -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn deribit_feed_print_5_sec() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();
        const WS_URL: &str = "wss://test.deribit.com/ws/api/v2";
        let (ws_stream, _) = tokio_tungstenite::connect_async(WS_URL)
            .await
            .expect("connect");
        let (mut write, mut read) = ws_stream.split();

        let subscribe_msg = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "method": "public/subscribe",
            "params": { "channels": [CHANNEL_TICKER, CHANNEL_TRADES, CHANNEL_PRICE_INDEX] },
            "id": 1
        }))
        .unwrap();
        write
            .send(Message::Text(subscribe_msg.into()))
            .await
            .expect("send subscribe");

        let run_for = std::time::Duration::from_secs(5);
        let deadline = tokio::time::Instant::now() + run_for;
        let mut count = 0u64;
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let msg = match timeout(remaining, read.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => text,
                Ok(Some(Ok(_))) => continue,
                Ok(Some(Err(e))) => {
                    eprintln!("WS error: {e}");
                    break;
                }
                Ok(None) => break,
                Err(_) => break,
            };
            let v: serde_json::Value = match serde_json::from_str(&msg) {
                Ok(x) => x,
                Err(_) => continue,
            };
            if v.get("method").and_then(|m| m.as_str()) == Some("subscription") {
                count += 1;
                let channel = v
                    .get("params")
                    .and_then(|p| p.get("channel"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("?");
                let data = v.get("params").and_then(|p| p.get("data"));
                println!(
                    "[{}] channel={} data={}",
                    count,
                    channel,
                    data.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "null".into())
                );
            }
        }
        println!("--- received {count} subscription messages in 5s ---");
    }
}

//! Deribit WebSocket client: JSON-RPC 2.0 connect, subscribe, read loop.

use crate::proto::market_data_message;
use crate::proto::MarketDataMessage;
use crate::source::deribit::constants::{PRODUCER_DERIBIT, VENUE_DERIBIT};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::{self, error::ProtocolError};
use tonic::Status;
use tracing::{error, info, warn};

const SUBSCRIBE_ID: u64 = 1;
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const CONNECTION_LIFETIME: Duration = Duration::from_secs(23 * 3600 + 30 * 60);
const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// All three Deribit channels for BTC options intelligence.
pub const CHANNEL_TICKER: &str = "ticker.BTC-OPTION.100ms";
pub const CHANNEL_TRADES: &str = "trades.option.BTC.100ms";
pub const CHANNEL_PRICE_INDEX: &str = "deribit_price_index.btc_usd";

/// Callback: (channel, data_json) -> zero or more data payloads to broadcast.
pub type OnNotification = Box<dyn Fn(&str, &str) -> Vec<market_data_message::Data> + Send + Sync>;

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
                let heartbeat_msg = serde_json::to_string(&json!({
                    "jsonrpc": "2.0",
                    "method": "public/set_heartbeat",
                    "params": {
                        "interval": HEARTBEAT_INTERVAL_SECS
                    },
                    "id": request_id
                }))
                .expect("set_heartbeat JSON");
                if let Err(e) = write.send(Message::Text(heartbeat_msg.into())).await {
                    warn!("Deribit: failed to send set_heartbeat request: {e}");
                } else {
                    info!(
                        "Deribit: requested websocket heartbeat interval={}s",
                        HEARTBEAT_INTERVAL_SECS
                    );
                }
                request_id = request_id.wrapping_add(1);

                let start = std::time::Instant::now();
                let mut should_reconnect = false;
                let mut notifications_seen: u64 = 0;
                let mut emitted_messages: u64 = 0;
                let mut empty_parse_logs: u8 = 0;
                let mut no_subscriber_logs: u8 = 0;

                while let Some(msg) = read.next().await {
                    if start.elapsed() >= CONNECTION_LIFETIME {
                        info!(
                            "Deribit: scheduled reconnection after {:?}",
                            CONNECTION_LIFETIME
                        );
                        should_reconnect = true;
                        break;
                    }
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Some((channel, data_str)) =
                                parse_subscription_notification(&text)
                            {
                                notifications_seen += 1;
                                let parsed = on_notification(channel.as_str(), data_str.as_str());
                                if parsed.is_empty() && empty_parse_logs < 5 {
                                    empty_parse_logs += 1;
                                    warn!(
                                        "Deribit: notification parsed to 0 messages for channel={channel}"
                                    );
                                }
                                for data in parsed {
                                    let msg = MarketDataMessage {
                                        venue: VENUE_DERIBIT.to_string(),
                                        producer: PRODUCER_DERIBIT.to_string(),
                                        data: Some(data),
                                    };
                                    emitted_messages += 1;
                                    if tx.send(Ok(msg)).is_err() {
                                        // No active gRPC subscribers yet; keep the WS session alive.
                                        if no_subscriber_logs < 3 {
                                            no_subscriber_logs += 1;
                                            info!(
                                                "Deribit: dropping message because there are no active subscribers yet"
                                            );
                                        }
                                    }
                                }
                                if notifications_seen == 1 {
                                    info!(
                                        "Deribit: first subscription notification received (channel={channel})"
                                    );
                                }
                                if notifications_seen % 500 == 0 {
                                    info!(
                                        "Deribit: notifications_seen={notifications_seen}, emitted_messages={emitted_messages}"
                                    );
                                }
                            } else if should_reply_public_test(&text) {
                                let test_msg = serde_json::to_string(&json!({
                                    "jsonrpc": "2.0",
                                    "method": "public/test",
                                    "id": request_id
                                }))
                                .expect("public/test JSON");
                                request_id = request_id.wrapping_add(1);
                                if let Err(e) = write.send(Message::Text(test_msg.into())).await {
                                    warn!("Deribit: failed to send public/test response: {e}");
                                    should_reconnect = true;
                                    break;
                                }
                                info!("Deribit: replied to heartbeat test_request with public/test");
                            } else {
                                log_control_message(&text);
                            }
                        }
                        Ok(Message::Ping(payload)) => {
                            // Keep the connection healthy by replying to server pings.
                            if let Err(e) = write.send(Message::Pong(payload)).await {
                                warn!("Deribit: failed to send pong: {e}");
                                should_reconnect = true;
                                break;
                            }
                        }
                        Ok(Message::Close(frame)) => {
                            warn!("Deribit: websocket closed by server: {frame:?}");
                            should_reconnect = true;
                            break;
                        }
                        Err(e) => {
                            if is_expected_disconnect(&e) {
                                warn!("Deribit: transient WS disconnect: {e}. Reconnecting...");
                            } else {
                                error!("Deribit: WS error: {e}");
                            }
                            should_reconnect = true;
                            break;
                        }
                        _ => {}
                    }
                }
                if !should_reconnect {
                    // The stream ended cleanly (`read.next() -> None`); treat as a disconnect.
                    warn!("Deribit: websocket stream ended; reconnecting...");
                    continue;
                }
            }
            Err(e) => {
                error!("Deribit: connect failed: {}", e);
            }
        }
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
}

/// Build per-instrument ticker channels for active BTC options.
///
/// Deribit currently does not reliably emit wildcard ticker channels for all options, so we
/// enumerate active instruments and subscribe to `ticker.<instrument>.100ms` for each.
pub async fn fetch_btc_option_ticker_channels(rest_url: &str) -> Vec<String> {
    let url = format!(
        "{}/public/get_instruments?currency=BTC&kind=option&expired=false",
        rest_url.trim_end_matches('/')
    );
    let response = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            warn!("Deribit: failed to fetch option instruments from REST: {e}");
            return vec![CHANNEL_TICKER.to_string()];
        }
    };
    let payload = match response.json::<Value>().await {
        Ok(v) => v,
        Err(e) => {
            warn!("Deribit: failed to decode get_instruments response: {e}");
            return vec![CHANNEL_TICKER.to_string()];
        }
    };
    let Some(arr) = payload.get("result").and_then(|v| v.as_array()) else {
        warn!("Deribit: get_instruments response missing result array");
        return vec![CHANNEL_TICKER.to_string()];
    };
    let mut out = Vec::with_capacity(arr.len());
    for instrument in arr {
        if let Some(name) = instrument.get("instrument_name").and_then(|v| v.as_str()) {
            out.push(format!("ticker.{name}.100ms"));
        }
    }
    if out.is_empty() {
        warn!("Deribit: no active BTC option instruments found");
        return vec![CHANNEL_TICKER.to_string()];
    }
    info!(
        "Deribit: fetched {} option instruments for ticker subscription",
        out.len()
    );
    out
}

fn log_control_message(text: &str) {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return;
    };
    if let Some(err) = v.get("error") {
        error!("Deribit: subscribe/control error response: {}", err);
        return;
    }
    if let (Some(id), Some(result)) = (v.get("id"), v.get("result")) {
        info!("Deribit: control response id={id}, result={result}");
    }
}

fn should_reply_public_test(text: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    v.get("method").and_then(|m| m.as_str()) == Some("heartbeat")
        && v.get("params")
            .and_then(|p| p.get("type"))
            .and_then(|t| t.as_str())
            == Some("test_request")
}

fn is_expected_disconnect(err: &tungstenite::Error) -> bool {
    matches!(
        err,
        tungstenite::Error::ConnectionClosed
            | tungstenite::Error::AlreadyClosed
            | tungstenite::Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)
            | tungstenite::Error::Io(_)
    )
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
            let Ok(Message::Text(text)) = msg else {
                continue;
            };
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
                    data.as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "null".into())
                );
            }
        }
        println!("--- received {count} subscription messages in 5s ---");
    }
}

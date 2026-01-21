use crate::proto::{market_data_message, MarketDataMessage};
use futures_util::{Sink, SinkExt, StreamExt};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tonic::Status;
use tracing::{error, info, warn};
use url::Url;

pub enum ShardCommand {
    Subscribe(Vec<String>),
    Unsubscribe(Vec<String>),
}

pub enum ControlKind {
    Subscribe,
    Unsubscribe,
}

pub struct RunShardArgs {
    pub shard_idx: usize,
    pub ws_url: String,
    pub interval: String,
    pub initial_symbols: Vec<String>,
    pub shard_size: usize,
    pub tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
    pub cmd_rx: mpsc::Receiver<ShardCommand>,
    pub streams_set: Arc<dashmap::DashSet<String>>,
    pub venue: String,
    pub producer: String,
    pub stream_name_for_symbol: fn(&str, &str) -> String,
    pub build_control_message: fn(ControlKind, &[String], u64) -> String,
    pub handle_text: fn(&str) -> Option<market_data_message::Data>,
    pub on_data: Option<fn(&market_data_message::Data)>,
    pub on_connect: Option<fn(usize)>,
    pub on_disconnect: Option<fn(usize)>,
    pub on_streams_seeded: Option<fn(usize, usize)>,
    pub on_connect_tx: Option<mpsc::Sender<usize>>,
    pub connection_lifetime: Duration,
    pub retry_interval: Duration,
}

pub async fn run_shard(args: RunShardArgs) {
    let RunShardArgs {
        shard_idx,
        ws_url,
        interval,
        initial_symbols,
        shard_size,
        tx,
        cmd_rx,
        streams_set,
        venue,
        producer,
        stream_name_for_symbol,
        build_control_message,
        handle_text,
        on_data,
        on_connect,
        on_disconnect,
        on_streams_seeded,
        on_connect_tx,
        connection_lifetime,
        retry_interval,
    } = args;

    let mut cmd_rx = cmd_rx;

    let url = match Url::parse(&ws_url) {
        Ok(u) => u,
        Err(e) => {
            error!("Shard {shard_idx}: invalid ws_url {ws_url}: {e}");
            return;
        }
    };

    // Seed initial subscriptions.
    for sym in initial_symbols.into_iter().take(shard_size) {
        streams_set.insert(stream_name_for_symbol(&sym, &interval));
    }
    if let Some(callback) = on_streams_seeded {
        callback(shard_idx, streams_set.len());
    }

    let next_id = AtomicU64::new((shard_idx as u64) * 10_000);

    loop {
        info!("Shard {shard_idx}: connecting to {url}");
        match tokio_tungstenite::connect_async(url.to_string()).await {
            Ok((ws_stream, _)) => {
                if let Some(callback) = on_connect {
                    callback(shard_idx);
                }
                if let Some(tx) = &on_connect_tx {
                    let _ = tx.send(shard_idx).await;
                }
                let (mut write, mut read) = ws_stream.split();

                // Initial subscribe (one message per shard to respect incoming-message limits).
                if !streams_set.is_empty() {
                    let params: Vec<String> = streams_set.iter().map(|s| s.key().clone()).collect();
                    if let Err(e) = send_control(
                        &mut write,
                        ControlKind::Subscribe,
                        &params,
                        &next_id,
                        build_control_message,
                    )
                    .await
                    {
                        warn!("Shard {shard_idx}: initial subscribe failed: {e}");
                    }
                }

                let mut last_control_send = Instant::now()
                    .checked_sub(Duration::from_secs(1))
                    .unwrap_or_else(Instant::now);
                let shard_loop = async {
                    loop {
                        tokio::select! {
                            Some(cmd) = cmd_rx.recv() => {
                                // Rate limit outgoing control messages: max ~10/sec.
                                let since = last_control_send.elapsed();
                                if since < Duration::from_millis(120) {
                                    tokio::time::sleep(Duration::from_millis(120) - since).await;
                                }

                                match cmd {
                                    ShardCommand::Subscribe(params) => {
                                        if !params.is_empty() {
                                            if let Err(e) =
                                                send_control(&mut write, ControlKind::Subscribe, &params, &next_id, build_control_message).await
                                            {
                                                warn!("Shard {shard_idx}: SUBSCRIBE failed: {e}");
                                                return true; // reconnect
                                            }
                                            last_control_send = Instant::now();
                                        }
                                    }
                                    ShardCommand::Unsubscribe(params) => {
                                        if !params.is_empty() {
                                            if let Err(e) =
                                                send_control(&mut write, ControlKind::Unsubscribe, &params, &next_id, build_control_message).await
                                            {
                                                warn!("Shard {shard_idx}: UNSUBSCRIBE failed: {e}");
                                                return true; // reconnect
                                            }
                                            last_control_send = Instant::now();
                                        }
                                    }
                                }
                            }
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                        if let Some(data) = handle_text(&text) {
                                            if let Some(callback) = on_data {
                                                callback(&data);
                                            }
                                            let m = MarketDataMessage {
                                                exchange: String::new(),
                                                venue: venue.clone(),
                                                producer: producer.clone(),
                                                data: Some(data),
                                            };
                                            let _ = tx.send(Ok(m));
                                        }
                                    }
                                    Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(_))) => {}
                                    Some(Ok(_)) => {}
                                    Some(Err(e)) => {
                                        warn!("Shard {shard_idx}: ws error: {e}");
                                        return true; // reconnect
                                    }
                                    None => {
                                        warn!("Shard {shard_idx}: ws ended");
                                        return true; // reconnect
                                    }
                                }
                            }
                            _ = tokio::time::sleep(connection_lifetime) => {
                                info!("Shard {shard_idx}: scheduled reconnect after {connection_lifetime:?}");
                                return true;
                            }
                        }
                    }
                };

                let should_reconnect = shard_loop.await;
                if let Some(callback) = on_disconnect {
                    callback(shard_idx);
                }
                if !should_reconnect {
                    break;
                }
            }
            Err(e) => {
                warn!("Shard {shard_idx}: connect failed: {e}");
            }
        }

        tokio::time::sleep(retry_interval).await;
    }
}

async fn send_control<W>(
    write: &mut W,
    kind: ControlKind,
    params: &[String],
    next_id: &AtomicU64,
    build_control_message: fn(ControlKind, &[String], u64) -> String,
) -> Result<(), tungstenite::Error>
where
    W: Sink<tokio_tungstenite::tungstenite::Message, Error = tungstenite::Error> + Unpin,
{
    let id = next_id.fetch_add(1, Ordering::Relaxed);
    let msg = build_control_message(kind, params, id);
    write
        .send(tokio_tungstenite::tungstenite::Message::Text(msg.into()))
        .await
}

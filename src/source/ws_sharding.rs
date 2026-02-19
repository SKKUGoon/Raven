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

type StreamNameForSymbolFn = fn(&str, &str) -> String;
type BuildControlMessageFn = fn(ControlKind, &[String], u64) -> String;
type HandleTextFn = fn(&str) -> Option<market_data_message::Data>;
type OnDataFn = fn(&market_data_message::Data);
type OnShardEventFn = fn(usize);
type OnStreamsSeededFn = fn(usize, usize);

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
    pub stream_name_for_symbol: StreamNameForSymbolFn,
    pub build_control_message: BuildControlMessageFn,
    pub handle_text: HandleTextFn,
    pub on_data: Option<OnDataFn>,
    pub on_connect: Option<OnShardEventFn>,
    pub on_disconnect: Option<OnShardEventFn>,
    pub on_streams_seeded: Option<OnStreamsSeededFn>,
    pub on_connect_tx: Option<mpsc::Sender<usize>>,
    pub connection_lifetime: Duration,
    pub retry_interval: Duration,
}

#[derive(Default)]
pub struct RunShardArgsBuilder {
    shard_idx: Option<usize>,
    ws_url: Option<String>,
    interval: Option<String>,
    initial_symbols: Vec<String>,
    shard_size: Option<usize>,
    tx: Option<broadcast::Sender<Result<MarketDataMessage, Status>>>,
    cmd_rx: Option<mpsc::Receiver<ShardCommand>>,
    streams_set: Option<Arc<dashmap::DashSet<String>>>,
    venue: Option<String>,
    producer: Option<String>,
    stream_name_for_symbol: Option<StreamNameForSymbolFn>,
    build_control_message: Option<BuildControlMessageFn>,
    handle_text: Option<HandleTextFn>,
    on_data: Option<OnDataFn>,
    on_connect: Option<OnShardEventFn>,
    on_disconnect: Option<OnShardEventFn>,
    on_streams_seeded: Option<OnStreamsSeededFn>,
    on_connect_tx: Option<mpsc::Sender<usize>>,
    connection_lifetime: Option<Duration>,
    retry_interval: Option<Duration>,
}

impl RunShardArgs {
    pub fn builder() -> RunShardArgsBuilder {
        RunShardArgsBuilder::default()
    }
}

impl RunShardArgsBuilder {
    pub fn shard_idx(mut self, shard_idx: usize) -> Self {
        self.shard_idx = Some(shard_idx);
        self
    }

    pub fn ws_url(mut self, ws_url: String) -> Self {
        self.ws_url = Some(ws_url);
        self
    }

    pub fn interval(mut self, interval: String) -> Self {
        self.interval = Some(interval);
        self
    }

    pub fn initial_symbols(mut self, initial_symbols: Vec<String>) -> Self {
        self.initial_symbols = initial_symbols;
        self
    }

    pub fn shard_size(mut self, shard_size: usize) -> Self {
        self.shard_size = Some(shard_size);
        self
    }

    pub fn tx(mut self, tx: broadcast::Sender<Result<MarketDataMessage, Status>>) -> Self {
        self.tx = Some(tx);
        self
    }

    pub fn cmd_rx(mut self, cmd_rx: mpsc::Receiver<ShardCommand>) -> Self {
        self.cmd_rx = Some(cmd_rx);
        self
    }

    pub fn streams_set(mut self, streams_set: Arc<dashmap::DashSet<String>>) -> Self {
        self.streams_set = Some(streams_set);
        self
    }

    pub fn venue(mut self, venue: String) -> Self {
        self.venue = Some(venue);
        self
    }

    pub fn producer(mut self, producer: String) -> Self {
        self.producer = Some(producer);
        self
    }

    pub fn stream_name_for_symbol(mut self, stream_name_for_symbol: StreamNameForSymbolFn) -> Self {
        self.stream_name_for_symbol = Some(stream_name_for_symbol);
        self
    }

    pub fn build_control_message(mut self, build_control_message: BuildControlMessageFn) -> Self {
        self.build_control_message = Some(build_control_message);
        self
    }

    pub fn handle_text(mut self, handle_text: HandleTextFn) -> Self {
        self.handle_text = Some(handle_text);
        self
    }

    pub fn on_data(mut self, on_data: Option<OnDataFn>) -> Self {
        self.on_data = on_data;
        self
    }

    pub fn on_connect(mut self, on_connect: Option<OnShardEventFn>) -> Self {
        self.on_connect = on_connect;
        self
    }

    pub fn on_disconnect(mut self, on_disconnect: Option<OnShardEventFn>) -> Self {
        self.on_disconnect = on_disconnect;
        self
    }

    pub fn on_streams_seeded(mut self, on_streams_seeded: Option<OnStreamsSeededFn>) -> Self {
        self.on_streams_seeded = on_streams_seeded;
        self
    }

    pub fn on_connect_tx(mut self, on_connect_tx: Option<mpsc::Sender<usize>>) -> Self {
        self.on_connect_tx = on_connect_tx;
        self
    }

    pub fn connection_lifetime(mut self, connection_lifetime: Duration) -> Self {
        self.connection_lifetime = Some(connection_lifetime);
        self
    }

    pub fn retry_interval(mut self, retry_interval: Duration) -> Self {
        self.retry_interval = Some(retry_interval);
        self
    }

    pub fn build(self) -> RunShardArgs {
        RunShardArgs {
            shard_idx: self.shard_idx.expect("missing shard_idx"),
            ws_url: self.ws_url.expect("missing ws_url"),
            interval: self.interval.expect("missing interval"),
            initial_symbols: self.initial_symbols,
            shard_size: self.shard_size.unwrap_or(1),
            tx: self.tx.expect("missing tx"),
            cmd_rx: self.cmd_rx.expect("missing cmd_rx"),
            streams_set: self.streams_set.expect("missing streams_set"),
            venue: self.venue.expect("missing venue"),
            producer: self.producer.expect("missing producer"),
            stream_name_for_symbol: self
                .stream_name_for_symbol
                .expect("missing stream_name_for_symbol"),
            build_control_message: self
                .build_control_message
                .expect("missing build_control_message"),
            handle_text: self.handle_text.expect("missing handle_text"),
            on_data: self.on_data,
            on_connect: self.on_connect,
            on_disconnect: self.on_disconnect,
            on_streams_seeded: self.on_streams_seeded,
            on_connect_tx: self.on_connect_tx,
            connection_lifetime: self
                .connection_lifetime
                .unwrap_or(Duration::from_secs(23 * 3600 + 30 * 60)),
            retry_interval: self.retry_interval.unwrap_or(Duration::from_secs(5)),
        }
    }
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
    build_control_message: BuildControlMessageFn,
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

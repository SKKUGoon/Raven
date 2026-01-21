use crate::config::{BinanceKlinesConfig, Settings};
use crate::proto::{market_data_message, MarketDataMessage};
use crate::service::{StreamDataType, StreamKey};
use crate::telemetry::{BINANCE_FUTURES_KLINES_CONNECTIONS, BINANCE_FUTURES_KLINES_PROCESSED};
use crate::telemetry::binance::{
    BINANCE_FUTURES_KLINES_SHARD_CONNECTIONS, BINANCE_FUTURES_KLINES_SHARD_STREAMS,
};
use crate::source::ws_sharding::{ControlKind, RunShardArgs, ShardCommand, run_shard};
use dashmap::DashSet;
use serde_json::json;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tonic::Status;
use tracing::{info, warn};

use super::symbols::resolve_symbols;

/// Binance Futures kline collector:
/// - Connects to `wss://fstream.binance.com/ws`
/// - Uses JSON `SUBSCRIBE` with multiple streams per connection
/// - Emits only **closed** klines (`k.x = true`) as `MarketDataMessage::Candle`
/// - Shards symbols across N connections with up to `shard_size` streams each.
#[derive(Clone)]
pub struct BinanceFuturesKlinesService {
    pub(super) cfg: BinanceKlinesConfig,
    pub(super) settings: Settings,
    pub(super) venue: String,
    pub(super) producer: String,
    pub(super) symbols: Arc<Vec<String>>,
    // Active collections: StreamKey -> subscriber_count (approx, from broadcast receiver_count)
    pub(super) active: Arc<DashSet<StreamKey>>,
    // All candles (for all symbols) get broadcast here; subscribers filter by symbol.
    pub(super) tx: broadcast::Sender<Result<MarketDataMessage, Status>>,
    // Per-shard command channel (subscribe/unsubscribe).
    pub(super) shard_cmds: Arc<Vec<mpsc::Sender<ShardCommand>>>,
    // Track current streams per shard (for capacity checks / list output).
    pub(super) shard_streams: Arc<Vec<Arc<DashSet<String>>>>,
}

impl BinanceFuturesKlinesService {
    pub async fn new(settings: &Settings) -> Self {
        let cfg = settings.binance_klines.clone();
        let settings = settings.clone();
        let venue = "BINANCE_FUTURES".to_string();
        let producer = "binance_futures_klines".to_string();

        let mut symbols = resolve_symbols(&settings).await;
        if symbols.is_empty() {
            warn!(
                "Binance futures klines: no symbols configured. Set `binance_klines.symbols` or add futures entries to `routing.symbol_map`."
            );
        }
        let max_initial = cfg.connections.max(1).saturating_mul(cfg.shard_size.max(1));
        if symbols.len() > max_initial {
            warn!(
                "Binance futures klines: {} symbols configured but only {} will be started initially (connections={} * shard_size={}).",
                symbols.len(),
                max_initial,
                cfg.connections.max(1),
                cfg.shard_size.max(1),
            );
            symbols.truncate(max_initial);
        }

        // Global broadcast channel: carries all candles; subscribers filter.
        let (tx, _) = broadcast::channel::<Result<MarketDataMessage, Status>>(cfg.channel_capacity);

        let shard_count = cfg.connections.max(1);
        let mut shard_cmds = Vec::with_capacity(shard_count);
        let mut shard_rxs = Vec::with_capacity(shard_count);
        let mut shard_streams = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            let (cmd_tx, cmd_rx) = mpsc::channel::<ShardCommand>(1024);
            shard_cmds.push(cmd_tx);
            shard_rxs.push(cmd_rx);
            shard_streams.push(Arc::new(DashSet::new()));
        }

        let active: Arc<DashSet<StreamKey>> = Arc::new(DashSet::new());
        for sym in &symbols {
            active.insert(StreamKey {
                symbol: sym.clone(),
                venue: Some(venue.clone()),
                data_type: StreamDataType::Candle,
            });
        }

        let service = Self {
            cfg,
            settings,
            venue,
            producer,
            symbols: Arc::new(symbols),
            active,
            tx,
            shard_cmds: Arc::new(shard_cmds),
            shard_streams: Arc::new(shard_streams),
        };

        let (refresh_tx, refresh_rx) = mpsc::channel::<usize>(16);
        service.spawn_shards(shard_rxs, refresh_tx);
        service.spawn_symbol_refresh(refresh_rx);
        service
    }

    fn spawn_shards(
        &self,
        shard_rxs: Vec<mpsc::Receiver<ShardCommand>>,
        refresh_tx: mpsc::Sender<usize>,
    ) {
        let base_url = self.cfg.ws_url.clone();
        let interval = self.cfg.interval.clone();
        let shard_size = self.cfg.shard_size.max(1);
        let shard_count = self.cfg.connections.max(1);

        // Initial set: cap to (connections * shard_size) so we never exceed per-connection stream limits.
        let max_initial = shard_count.saturating_mul(shard_size);
        if self.symbols.len() > max_initial {
            warn!(
                "Binance futures klines: {} symbols configured, but only {} can be subscribed initially (connections={} * shard_size={}). Remaining symbols require Control.StartCollection (dynamic subscribe) or increasing config.",
                self.symbols.len(),
                max_initial,
                shard_count,
                shard_size
            );
        }

        // Build contiguous chunks (50 at a time), then assign chunk i -> shard (i % shard_count).
        // This matches your \"50 per connection\" mental model but still works if shard_count differs.
        let mut assigned: Vec<Vec<String>> = vec![Vec::new(); shard_count];
        let initial: Vec<String> = self.symbols.iter().take(max_initial).cloned().collect();
        for (i, chunk) in initial.chunks(shard_size).enumerate() {
            let shard_idx = i % shard_count;
            assigned[shard_idx].extend(chunk.iter().cloned());
        }

        let mut rx_iter = shard_rxs.into_iter();
        for shard_idx in 0..shard_count {
            let initial_symbols = assigned.get(shard_idx).cloned().unwrap_or_default();
            let tx = self.tx.clone();
            let cmd_rx = rx_iter.next().unwrap_or_else(|| {
                let (_tx, rx) = mpsc::channel::<ShardCommand>(1);
                rx
            });

            let streams_set = self.shard_streams[shard_idx].clone();
            let venue = self.venue.clone();
            let producer = self.producer.clone();
            let ws_url = base_url.clone();
            let ws_interval = interval.clone();
            let refresh_tx = refresh_tx.clone();

            tokio::spawn(async move {
                run_shard(RunShardArgs {
                    shard_idx,
                    ws_url,
                    interval: ws_interval,
                    initial_symbols,
                    shard_size,
                    tx,
                    cmd_rx,
                    streams_set,
                    venue,
                    producer,
                    stream_name_for_symbol: build_stream_name,
                    build_control_message,
                    handle_text: handle_kline_text,
                    on_data: Some(on_kline_data),
                    on_connect: Some(on_kline_shard_connect),
                    on_disconnect: Some(on_kline_shard_disconnect),
                    on_streams_seeded: Some(on_kline_shard_streams_seeded),
                    on_connect_tx: Some(refresh_tx),
                    connection_lifetime: std::time::Duration::from_secs(23 * 3600 + 30 * 60),
                    retry_interval: std::time::Duration::from_secs(5),
                })
                .await
            });
        }
    }

    fn spawn_symbol_refresh(&self, mut refresh_rx: mpsc::Receiver<usize>) {
        let service = self.clone();
        tokio::spawn(async move {
            let mut last_refresh = Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now);
            while let Some(shard_idx) = refresh_rx.recv().await {
                if last_refresh.elapsed() < Duration::from_secs(30) {
                    continue;
                }
                last_refresh = Instant::now();
                info!("Klines shard {shard_idx} connected; refreshing symbol list");
                service.refresh_symbols().await;
            }
        });
    }

    async fn refresh_symbols(&self) {
        let desired = resolve_symbols(&self.settings).await;
        if desired.is_empty() {
            warn!("Binance futures klines: symbol refresh returned empty list.");
            return;
        }

        let desired_set: HashSet<String> = desired.into_iter().collect();
        let current_set: HashSet<String> =
            self.active.iter().map(|k| k.key().symbol.clone()).collect();

        for sym in desired_set.difference(&current_set) {
            if let Err(e) = self.start_symbol(sym).await {
                warn!("Failed to subscribe symbol {sym}: {e}");
            }
        }

        for sym in current_set.difference(&desired_set) {
            if let Err(e) = self.stop_symbol(sym).await {
                warn!("Failed to unsubscribe symbol {sym}: {e}");
            }
        }
    }

    pub(super) fn key_for_symbol(&self, symbol: &str) -> StreamKey {
        StreamKey {
            symbol: symbol.trim().to_uppercase(),
            venue: Some(self.venue.clone()),
            data_type: StreamDataType::Candle,
        }
    }

    fn shard_for_symbol(&self, symbol: &str) -> usize {
        let shard_count = self.cfg.connections.max(1);
        let mut hasher = DefaultHasher::new();
        symbol.to_uppercase().hash(&mut hasher);
        (hasher.finish() as usize) % shard_count
    }

    pub(super) async fn start_symbol(&self, symbol: &str) -> Result<(), Status> {
        let symbol = symbol.trim().to_uppercase();
        if symbol.is_empty() {
            return Err(Status::invalid_argument("symbol is empty"));
        }

        let key = self.key_for_symbol(&symbol);
        self.active.insert(key);

        let shard_idx = self.shard_for_symbol(&symbol);
        let stream_name = build_stream_name(&symbol, &self.cfg.interval);

        // Capacity checks (Binance hard limit 1024 streams/connection)
        if self.shard_streams[shard_idx].len() >= self.cfg.max_streams_per_connection {
            return Err(Status::resource_exhausted(format!(
                "Shard {shard_idx} already has {} streams (max {})",
                self.shard_streams[shard_idx].len(),
                self.cfg.max_streams_per_connection
            )));
        }

        // If already present, no-op.
        if self.shard_streams[shard_idx].insert(stream_name.clone()) {
            let cmd = ShardCommand::Subscribe(vec![stream_name]);
            self.shard_cmds[shard_idx]
                .send(cmd)
                .await
                .map_err(|_| Status::unavailable("Shard command channel closed"))?;
        }
        set_kline_shard_streams(shard_idx, self.shard_streams[shard_idx].len());

        Ok(())
    }

    pub(super) async fn stop_symbol(&self, symbol: &str) -> Result<(), Status> {
        let symbol = symbol.trim().to_uppercase();
        let key = self.key_for_symbol(&symbol);
        self.active.remove(&key);

        let shard_idx = self.shard_for_symbol(&symbol);
        let stream_name = build_stream_name(&symbol, &self.cfg.interval);
        if self.shard_streams[shard_idx].remove(&stream_name).is_some() {
            let cmd = ShardCommand::Unsubscribe(vec![stream_name]);
            self.shard_cmds[shard_idx]
                .send(cmd)
                .await
                .map_err(|_| Status::unavailable("Shard command channel closed"))?;
        }
        set_kline_shard_streams(shard_idx, self.shard_streams[shard_idx].len());

        Ok(())
    }

    pub(super) fn active_keys(&self) -> Vec<StreamKey> {
        self.active.iter().map(|k| k.key().clone()).collect()
    }

    pub(super) fn is_active(&self, key: &StreamKey) -> bool {
        self.active.contains(key)
    }

    pub(super) fn tx(&self) -> broadcast::Sender<Result<MarketDataMessage, Status>> {
        self.tx.clone()
    }

    pub(super) fn venue(&self) -> &str {
        &self.venue
    }
}

fn build_stream_name(symbol: &str, interval: &str) -> String {
    format!("{}@kline_{}", symbol.trim().to_lowercase(), interval.trim())
}

fn build_control_message(kind: ControlKind, params: &[String], id: u64) -> String {
    let method = match kind {
        ControlKind::Subscribe => "SUBSCRIBE",
        ControlKind::Unsubscribe => "UNSUBSCRIBE",
    };
    json!({
        "method": method,
        "params": params,
        "id": id
    })
    .to_string()
}

fn handle_kline_text(text: &str) -> Option<market_data_message::Data> {
    crate::source::binance::futures::parse_binance_futures_candle(text, "")
}

fn on_kline_data(data: &market_data_message::Data) {
    if let market_data_message::Data::Candle(c) = data {
        BINANCE_FUTURES_KLINES_PROCESSED.with_label_values(&[&c.symbol]).inc();
    }
}

fn on_kline_shard_connect(shard_idx: usize) {
    BINANCE_FUTURES_KLINES_CONNECTIONS.inc();
    set_kline_shard_connection(shard_idx, true);
}

fn on_kline_shard_disconnect(shard_idx: usize) {
    BINANCE_FUTURES_KLINES_CONNECTIONS.dec();
    set_kline_shard_connection(shard_idx, false);
}

fn on_kline_shard_streams_seeded(shard_idx: usize, count: usize) {
    set_kline_shard_streams(shard_idx, count);
}

fn set_kline_shard_connection(shard_idx: usize, connected: bool) {
    let shard_label = shard_idx.to_string();
    BINANCE_FUTURES_KLINES_SHARD_CONNECTIONS
        .with_label_values(&[shard_label.as_str()])
        .set(if connected { 1 } else { 0 });
}

fn set_kline_shard_streams(shard_idx: usize, count: usize) {
    let shard_label = shard_idx.to_string();
    BINANCE_FUTURES_KLINES_SHARD_STREAMS
        .with_label_values(&[shard_label.as_str()])
        .set(count as i64);
}

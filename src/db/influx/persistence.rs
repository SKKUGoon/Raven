use crate::proto::market_data_client::MarketDataClient;
use crate::proto::{market_data_message, DataType, MarketDataRequest};
use crate::telemetry::{INFLUX_ACTIVE_TASKS, INFLUX_POINTS_WRITTEN};
use influxdb2::models::DataPoint;
use influxdb2::Client;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};

struct InfluxActiveGuard;
impl Drop for InfluxActiveGuard {
    fn drop(&mut self) {
        INFLUX_ACTIVE_TASKS.dec();
    }
}

pub(super) struct PersistenceRunConfig {
    pub upstream_url: String,
    pub symbol: String,
    pub exchange: String,
    pub key: String,
    pub data_type: DataType,
    pub influx_client: Client,
    pub bucket: String,
    pub batch_size: usize,
    pub batch_interval_ms: u64,
}

impl PersistenceRunConfig {
    pub async fn run(self) {
        let Self {
            upstream_url,
            symbol,
            exchange,
            key,
            data_type,
            influx_client,
            bucket,
            batch_size,
            batch_interval_ms,
        } = self;

        let key_clone = key.clone();
    INFLUX_ACTIVE_TASKS.inc();
    let _active_guard = InfluxActiveGuard;

    info!("Persistence task started for {}", key_clone);

    // Create a channel to decouple reading from writing
    // High capacity to handle bursts
    let (tx, mut rx) = mpsc::channel::<DataPoint>(10000);

    // Spawn writer task
    let writer_client = influx_client.clone();
    let writer_bucket = bucket.clone();
    let writer_key = key_clone.clone();

    let _writer_handle = tokio::spawn(async move {
        let mut buffer: Vec<DataPoint> = Vec::with_capacity(batch_size);
        let mut interval = time::interval(Duration::from_millis(batch_interval_ms));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                Some(point) = rx.recv() => {
                    buffer.push(point);
                    if buffer.len() >= batch_size {
                        flush_buffer(&writer_client, &writer_bucket, &mut buffer).await;
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        flush_buffer(&writer_client, &writer_bucket, &mut buffer).await;
                    }
                }
                else => {
                    // Channel closed
                    break;
                }
            }
        }
        // Flush remaining
        if !buffer.is_empty() {
            flush_buffer(&writer_client, &writer_bucket, &mut buffer).await;
        }
        info!("Persistence writer task ended for {}", writer_key);
    });

    loop {
        // (Re)connect + (re)subscribe. This will initially fail until the upstream stream
        // has been started via Control, which supports "wire first, subscribe later".
        let mut client = match MarketDataClient::connect(upstream_url.clone()).await {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "Failed to connect to upstream {}: {}. Retrying in 2s...",
                    upstream_url, e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        let request = tonic::Request::new(MarketDataRequest {
            symbol: symbol.clone(),
            data_type: data_type as i32,
            venue: exchange.clone(),
        });

        let mut stream = match client.subscribe(request).await {
            Ok(res) => res.into_inner(),
            Err(e) => {
                warn!(
                    "Failed to subscribe to upstream ({}): {}. Retrying in 2s...",
                    key_clone, e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        // Reader loop - purely reads and pushes to channel
        loop {
            match stream.message().await {
                Ok(Some(msg)) => {
                    if let Some(point) = msg_to_datapoint(&msg, &exchange) {
                        if tx.try_send(point).is_err() {
                            warn!("Persistence buffer full for {}. Dropping point.", key_clone);
                        }
                    }
                }
                Ok(None) => {
                    warn!(
                        "Upstream stream ended for {}. Reconnecting in 2s...",
                        key_clone
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    break;
                }
                Err(e) => {
                    warn!(
                        "Error receiving message for {}: {}. Reconnecting in 2s...",
                        key_clone, e
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    break;
                }
            }
        }
    }
    }
}

fn msg_to_datapoint(msg: &crate::proto::MarketDataMessage, exchange: &str) -> Option<DataPoint> {
    match msg.data.as_ref()? {
        market_data_message::Data::Trade(trade) => {
            let ts = trade.timestamp.saturating_mul(1_000_000);
            let mut pb = DataPoint::builder("trades")
                .tag("symbol", &trade.symbol)
                .tag("side", &trade.side)
                .field("price", trade.price)
                .field("quantity", trade.quantity)
                .timestamp(ts);
            if !exchange.is_empty() {
                pb = pb.tag("exchange", exchange);
            }
            pb.build().ok()
        }
        market_data_message::Data::Orderbook(book) => {
            let ts = book.timestamp.saturating_mul(1_000_000);
            let best_bid = book.bids.first();
            let best_ask = book.asks.first();

            let mut pb = DataPoint::builder("orderbook")
                .tag("symbol", &book.symbol)
                .timestamp(ts);
            if !exchange.is_empty() {
                pb = pb.tag("exchange", exchange);
            }
            if let Some(bid) = best_bid {
                pb = pb.field("bid_price", bid.price).field("bid_qty", bid.quantity);
            }
            if let Some(ask) = best_ask {
                pb = pb.field("ask_price", ask.price).field("ask_qty", ask.quantity);
            }
            if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                let spread = ask.price - bid.price;
                let mid = (ask.price + bid.price) / 2.0;
                let imbalance = bid.quantity / (bid.quantity + ask.quantity);
                pb = pb
                    .field("spread", spread)
                    .field("mid_price", mid)
                    .field("imbalance", imbalance);
            }
            pb.build().ok()
        }
        market_data_message::Data::Funding(f) => {
            let ts = f.timestamp.saturating_mul(1_000_000);
            let mut pb = DataPoint::builder("funding_rate")
                .tag("symbol", &f.symbol)
                .field("rate", f.rate)
                .field("mark_price", f.mark_price)
                .field("index_price", f.index_price)
                .field("next_funding_time", f.next_funding_time)
                .timestamp(ts);
            if !exchange.is_empty() {
                pb = pb.tag("exchange", exchange);
            }
            pb.build().ok()
        }
        market_data_message::Data::Liquidation(l) => {
            let ts = l.timestamp.saturating_mul(1_000_000);
            let mut pb = DataPoint::builder("liquidation")
                .tag("symbol", &l.symbol)
                .tag("side", &l.side)
                .field("price", l.price)
                .field("quantity", l.quantity)
                .timestamp(ts);
            if !exchange.is_empty() {
                pb = pb.tag("exchange", exchange);
            }
            pb.build().ok()
        }
        market_data_message::Data::OpenInterest(oi) => {
            let ts = oi.timestamp.saturating_mul(1_000_000);
            let mut pb = DataPoint::builder("open_interest")
                .tag("symbol", &oi.symbol)
                .field("open_interest", oi.open_interest)
                .field("open_interest_value", oi.open_interest_value)
                .timestamp(ts);
            if !exchange.is_empty() {
                pb = pb.tag("exchange", exchange);
            }
            pb.build().ok()
        }
        market_data_message::Data::OptionsTicker(t) => {
            let ts = t.timestamp.saturating_mul(1_000_000);
            let mut pb = DataPoint::builder("options_ticker")
                .tag("symbol", &t.symbol)
                .field("open_interest", t.open_interest)
                .field("mark_iv", t.mark_iv)
                .field("best_bid_price", t.best_bid_price)
                .field("best_ask_price", t.best_ask_price)
                .field("mark_price", t.mark_price)
                .field("underlying_price", t.underlying_price)
                .field("last_price", t.last_price)
                .field("bid_iv", t.bid_iv)
                .field("ask_iv", t.ask_iv)
                .timestamp(ts);
            if !exchange.is_empty() {
                pb = pb.tag("exchange", exchange);
            }
            pb.build().ok()
        }
        market_data_message::Data::PriceIndex(p) => {
            let ts = p.timestamp.saturating_mul(1_000_000);
            let mut pb = DataPoint::builder("price_index")
                .tag("index_name", &p.index_name)
                .field("price", p.price)
                .timestamp(ts);
            if !exchange.is_empty() {
                pb = pb.tag("exchange", exchange);
            }
            pb.build().ok()
        }
        _ => None,
    }
}

async fn flush_buffer(client: &Client, bucket: &str, buffer: &mut Vec<DataPoint>) {
    if buffer.is_empty() {
        return;
    }

    let mut retries = 0;
    const MAX_RETRIES: u32 = 3;
    let mut backoff = Duration::from_millis(100);

    loop {
        let points = buffer.clone();

        if let Err(e) = client.write(bucket, tokio_stream::iter(points)).await {
            retries += 1;
            if retries > MAX_RETRIES {
                error!(
                    "PERSISTENT FAILURE writing to InfluxDB after {} retries. Dropping {} points. Error: {}",
                    MAX_RETRIES,
                    buffer.len(),
                    e
                );
                buffer.clear();
                return;
            }
            warn!("Failed to write to InfluxDB (retry {retries}/{MAX_RETRIES}): {e}. Retrying in {backoff:?}...");
            time::sleep(backoff).await;
            backoff *= 2;
        } else {
            let count = buffer.len();
            INFLUX_POINTS_WRITTEN
                .with_label_values(&[bucket])
                .inc_by(count as u64);
            buffer.clear();
            return;
        }
    }
}

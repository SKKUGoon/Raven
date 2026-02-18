use crate::config::TimescaleConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::{market_data_message, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamKey, StreamManager, StreamWorker};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tonic::Request;
use tonic::Status;
use tracing::{error, info, warn};

use super::dim_cache::DimCache;
use super::schema::{ensure_schema_and_tables, qualify_table, validate_pg_ident};

#[derive(Clone)]
pub struct TimescaleWorker {
    tibs_upstreams: Vec<String>,
    vibs_upstreams: Vec<String>,
    vpin_upstreams: Vec<String>,
    schema: String,
    pool: Pool<Postgres>,
    dim_cache: Arc<DimCache>,
}

#[tonic::async_trait]
impl StreamWorker for TimescaleWorker {
    async fn run(&self, key: StreamKey, _tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        let symbol = key.symbol.clone();
        let venue = key.venue.clone().unwrap_or_default();

        run_multi_persistence(MultiPersistenceArgs {
            tibs_upstreams: self.tibs_upstreams.clone(),
            vibs_upstreams: self.vibs_upstreams.clone(),
            vpin_upstreams: self.vpin_upstreams.clone(),
            symbol,
            venue,
            key: key.to_string(),
            schema: self.schema.clone(),
            pool: self.pool.clone(),
            dim_cache: self.dim_cache.clone(),
        })
        .await
    }
}

pub type PersistenceService = StreamManager<TimescaleWorker>;

pub async fn new(
    tibs_upstreams: Vec<String>,
    vibs_upstreams: Vec<String>,
    vpin_upstreams: Vec<String>,
    config: TimescaleConfig,
) -> Result<PersistenceService, sqlx::Error> {
    let schema = validate_pg_ident(&config.schema)
        .unwrap_or_else(|| {
            warn!(
                "Invalid timescale.schema `{}` (must match [A-Za-z_][A-Za-z0-9_]*); falling back to `mart`",
                config.schema
            );
            "mart"
        })
        .to_string();

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.url)
        .await?;

    let tib_table = qualify_table(&schema, "fact__tick_imbalance");
    let vib_table = qualify_table(&schema, "fact__volume_imbalance");
    let vpin_table = qualify_table(&schema, "fact__vpin");

    if let Err(e) = ensure_schema_and_tables(
        &pool,
        &schema,
        &tib_table,
        &vib_table,
        &vpin_table,
    )
    .await
    {
        warn!(
            "Failed to create/verify hypertable (might already exist or not using TimescaleDB): {}",
            e
        );
    }

    let worker = TimescaleWorker {
        tibs_upstreams,
        vibs_upstreams,
        vpin_upstreams,
        schema,
        pool,
        dim_cache: Arc::new(DimCache::new()),
    };

    Ok(StreamManager::new(Arc::new(worker), 10000, false))
}

async fn connect_candle_stream(
    upstream_url: &str,
    symbol: &str,
    venue: &str,
    key: &str,
) -> tonic::Streaming<MarketDataMessage> {
    loop {
        let mut client = match MarketDataClient::connect(upstream_url.to_string()).await {
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

        let request = Request::new(MarketDataRequest {
            symbol: symbol.to_string(),
            data_type: DataType::Candle as i32,
            venue: venue.to_string(),
        });

        match client.subscribe(request).await {
            Ok(res) => return res.into_inner(),
            Err(e) => {
                // With "wire first, subscribe later", this can fail until the producer stream is started via Control.
                warn!(
                    "Failed to subscribe to candles ({} -> {}): {}. Retrying in 2s...",
                    key, upstream_url, e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        }
    }
}

async fn persist_candle(
    dim_cache: &DimCache,
    pool: &Pool<Postgres>,
    schema: &str,
    venue: &str,
    key: &str,
    candle: crate::proto::Candle,
) {
    let time = chrono::DateTime::from_timestamp_millis(candle.timestamp);

    if let Some(t) = time {
        let table_name = if candle.interval.starts_with("vpin") {
            qualify_table(schema, "fact__vpin")
        } else if candle.interval.starts_with("tib") || candle.interval.starts_with("trb") {
            qualify_table(schema, "fact__tick_imbalance")
        } else if candle.interval.starts_with("vib") {
            qualify_table(schema, "fact__volume_imbalance")
        } else {
            warn!("Unknown candle interval prefix '{}'; skipping persistence", candle.interval);
            return;
        };

        let (coin_id, quote_id) = match dim_cache.resolve_coin_quote(pool, schema, &candle.symbol).await
        {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to resolve coin/quote dimensions for {}: {}", key, e);
                return;
            }
        };
        let exchange_id = match dim_cache.resolve_exchange(pool, schema, venue).await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to resolve exchange dimension for {}: {}", key, e);
                return;
            }
        };
        let interval_id = match dim_cache
            .resolve_interval(pool, schema, &candle.interval)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to resolve interval dimension for {}: {}", key, e);
                return;
            }
        };

        let query = format!(
            r#"
            INSERT INTO {table_name} (time, coin_id, quote_id, exchange_id, interval_id, open, high, low, close, volume, buy_ticks, sell_ticks, total_ticks, theta)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        );

        let result = sqlx::query(&query)
            .bind(t)
            .bind(coin_id)
            .bind(quote_id)
            .bind(exchange_id)
            .bind(interval_id)
            .bind(candle.open)
            .bind(candle.high)
            .bind(candle.low)
            .bind(candle.close)
            .bind(candle.volume)
            .bind(candle.buy_ticks as i64)
            .bind(candle.sell_ticks as i64)
            .bind(candle.total_ticks as i64)
            .bind(candle.theta)
            .execute(pool)
            .await;

        if let Err(e) = result {
            error!("Failed to insert candle for {}: {}", key, e);
        }
    }
}

struct PersistenceLoopConfig {
    upstream_url: String,
    symbol: String,
    venue: String,
    key: String,
    schema: String,
    pool: Pool<Postgres>,
    dim_cache: Arc<DimCache>,
    stream_name: &'static str,
}

impl PersistenceLoopConfig {
    async fn run(self) {
        let Self {
            upstream_url,
            symbol,
            venue,
            key,
            schema,
            pool,
            dim_cache,
            stream_name,
        } = self;

        let mut stream = connect_candle_stream(&upstream_url, &symbol, &venue, &key).await;
        loop {
            match stream.message().await {
                Ok(Some(m)) => {
                    if let Some(market_data_message::Data::Candle(candle)) = m.data {
                        persist_candle(&dim_cache, &pool, &schema, &venue, &key, candle).await;
                    }
                }
                Ok(None) => {
                    warn!(
                        "{stream_name} stream ended for {}. Reconnecting in 2s...",
                        key
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    stream = connect_candle_stream(&upstream_url, &symbol, &venue, &key).await;
                }
                Err(e) => {
                    warn!(
                        "{stream_name} stream error for {}: {}. Reconnecting in 2s...",
                        key, e
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    stream = connect_candle_stream(&upstream_url, &symbol, &venue, &key).await;
                }
            }
        }
    }
}

struct MultiPersistenceArgs {
    tibs_upstreams: Vec<String>,
    vibs_upstreams: Vec<String>,
    vpin_upstreams: Vec<String>,
    symbol: String,
    venue: String,
    key: String,
    schema: String,
    pool: Pool<Postgres>,
    dim_cache: Arc<DimCache>,
}

async fn run_multi_persistence(args: MultiPersistenceArgs) {
    let MultiPersistenceArgs {
        tibs_upstreams,
        vibs_upstreams,
        vpin_upstreams,
        symbol,
        venue,
        key,
        schema,
        pool,
        dim_cache,
    } = args;
    info!("Bar Persistence task started for {}", key);

    let mut tib_tasks = Vec::with_capacity(tibs_upstreams.len());
    for (i, upstream) in tibs_upstreams.into_iter().enumerate() {
        let t_key = key.clone();
        let t_schema = schema.clone();
        let t_pool = pool.clone();
        let t_dim = dim_cache.clone();
        let t_symbol = symbol.clone();
        let t_venue = venue.clone();
        let label: &'static str = match i {
            0 => "Tibs(0)",
            1 => "Tibs(1)",
            2 => "Tibs(2)",
            _ => "Tibs",
        };
        tib_tasks.push(tokio::spawn(async move {
            PersistenceLoopConfig {
                upstream_url: upstream,
                symbol: t_symbol,
                venue: t_venue,
                key: t_key,
                schema: t_schema,
                pool: t_pool,
                dim_cache: t_dim,
                stream_name: label,
            }
            .run()
            .await;
        }));
    }

    let mut vib_tasks = Vec::with_capacity(vibs_upstreams.len());
    for (i, upstream) in vibs_upstreams.into_iter().enumerate() {
        let t_key = key.clone();
        let t_schema = schema.clone();
        let t_pool = pool.clone();
        let t_dim = dim_cache.clone();
        let t_symbol = symbol.clone();
        let t_venue = venue.clone();
        let label: &'static str = match i {
            0 => "Vibs(0)",
            1 => "Vibs(1)",
            2 => "Vibs(2)",
            _ => "Vibs",
        };
        vib_tasks.push(tokio::spawn(async move {
            PersistenceLoopConfig {
                upstream_url: upstream,
                symbol: t_symbol,
                venue: t_venue,
                key: t_key,
                schema: t_schema,
                pool: t_pool,
                dim_cache: t_dim,
                stream_name: label,
            }
            .run()
            .await;
        }));
    }

    let mut vpin_tasks = Vec::with_capacity(vpin_upstreams.len());
    for (i, upstream) in vpin_upstreams.into_iter().enumerate() {
        let t_key = key.clone();
        let t_schema = schema.clone();
        let t_pool = pool.clone();
        let t_dim = dim_cache.clone();
        let t_symbol = symbol.clone();
        let t_venue = venue.clone();
        let label: &'static str = match i {
            0 => "VPIN(0)",
            _ => "VPIN",
        };
        vpin_tasks.push(tokio::spawn(async move {
            PersistenceLoopConfig {
                upstream_url: upstream,
                symbol: t_symbol,
                venue: t_venue,
                key: t_key,
                schema: t_schema,
                pool: t_pool,
                dim_cache: t_dim,
                stream_name: label,
            }
            .run()
            .await;
        }));
    }

    for t in tib_tasks {
        let _ = t.await;
    }
    for t in vib_tasks {
        let _ = t.await;
    }
    for t in vpin_tasks {
        let _ = t.await;
    }
}

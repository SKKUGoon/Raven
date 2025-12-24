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

#[derive(Clone)]
pub struct TimescaleWorker {
    timebar_upstream: String,
    tibs_upstream: String,
    schema: String,
    pool: Pool<Postgres>,
}

#[tonic::async_trait]
impl StreamWorker for TimescaleWorker {
    async fn run(&self, key: StreamKey, _tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        let symbol = key.symbol.clone();
        let venue = key.venue.clone().unwrap_or_default();

        run_dual_persistence(
            self.timebar_upstream.clone(),
            self.tibs_upstream.clone(),
            symbol,
            venue,
            key.to_string(),
            self.schema.clone(),
            self.pool.clone(),
        )
        .await
    }
}

pub type PersistenceService = StreamManager<TimescaleWorker>;

pub async fn new(
    timebar_upstream: String,
    tibs_upstream: String,
    config: TimescaleConfig,
) -> Result<PersistenceService, sqlx::Error> {
    let schema = validate_pg_ident(&config.schema)
        .unwrap_or_else(|| {
            warn!(
                "Invalid timescale.schema `{}` (must match [A-Za-z_][A-Za-z0-9_]*); falling back to `warehouse`",
                config.schema
            );
            "warehouse"
        })
        .to_string();

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.url)
        .await?;

    let time_table = qualify_table(&schema, "bar__time");
    let tib_table = qualify_table(&schema, "bar__tick_imbalance");

    // Ensure the table exists (basic migration)
    // In a production env, use proper migrations.
    if let Err(e) = ensure_schema_and_tables(&pool, &schema, &time_table, &tib_table).await {
        warn!(
            "Failed to create/verify hypertable (might already exist or not using TimescaleDB): {}",
            e
        );
    }

    let worker = TimescaleWorker {
        timebar_upstream,
        tibs_upstream,
        schema,
        pool,
    };

    Ok(StreamManager::new(Arc::new(worker), 10000, false))
}

fn validate_pg_ident(s: &str) -> Option<&str> {
    let mut chars = s.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(s)
}

fn qualify_table(schema: &str, table: &str) -> String {
    format!("{schema}.{table}")
}

async fn ensure_schema_and_tables(
    pool: &Pool<Postgres>,
    schema: &str,
    time_table: &str,
    tib_table: &str,
) -> Result<(), sqlx::Error> {
    // Postgres drivers generally disallow multiple statements in a single prepared statement,
    // so keep this strictly one statement per execute().
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema};"))
        .execute(pool)
        .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {tib_table} (
            time        TIMESTAMPTZ NOT NULL,
            symbol      TEXT NOT NULL,
            exchange    TEXT NOT NULL,
            interval    TEXT NOT NULL,
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            buy_ticks   BIGINT NOT NULL DEFAULT 0,
            sell_ticks  BIGINT NOT NULL DEFAULT 0,
            total_ticks BIGINT NOT NULL DEFAULT 0,
            theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0
        );
        "#
    ))
    .execute(pool)
    .await?;

    // If TimescaleDB isn't installed/enabled, this will error; that's okay.
    // (Callers treat this as best-effort setup.)
    sqlx::query(&format!(
        "SELECT create_hypertable('{tib_table}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {time_table} (
            time        TIMESTAMPTZ NOT NULL,
            symbol      TEXT NOT NULL,
            exchange    TEXT NOT NULL,
            interval    TEXT NOT NULL,
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            buy_ticks   BIGINT NOT NULL DEFAULT 0,
            sell_ticks  BIGINT NOT NULL DEFAULT 0,
            total_ticks BIGINT NOT NULL DEFAULT 0,
            theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "SELECT create_hypertable('{time_table}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    Ok(())
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
    pool: &Pool<Postgres>,
    schema: &str,
    venue: &str,
    key: &str,
    candle: crate::proto::Candle,
) {
    let time = chrono::DateTime::from_timestamp_millis(candle.timestamp);

    if let Some(t) = time {
        let table_name = if candle.interval == "tib" {
            qualify_table(schema, "bar__tick_imbalance")
        } else {
            qualify_table(schema, "bar__time")
        };

        let query = format!(
            r#"
            INSERT INTO {table_name} (time, symbol, exchange, interval, open, high, low, close, volume, buy_ticks, sell_ticks, total_ticks, theta)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        );

        let result = sqlx::query(&query)
            .bind(t)
            .bind(&candle.symbol)
            .bind(venue)
            .bind(&candle.interval)
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

async fn run_dual_persistence(
    timebar_upstream: String,
    tibs_upstream: String,
    symbol: String,
    venue: String,
    key: String, // For logging
    schema: String,
    pool: Pool<Postgres>,
) {
    info!("Bar Persistence task started for {}", key);

    let mut timebar_stream = connect_candle_stream(&timebar_upstream, &symbol, &venue, &key).await;
    let mut tibs_stream = connect_candle_stream(&tibs_upstream, &symbol, &venue, &key).await;

    loop {
        tokio::select! {
            msg = timebar_stream.message() => {
                match msg {
                    Ok(Some(m)) => {
                        if let Some(market_data_message::Data::Candle(candle)) = m.data {
                            persist_candle(&pool, &schema, &venue, &key, candle).await;
                        }
                    }
                    Ok(None) => {
                        warn!("Timebar stream ended for {}. Reconnecting in 2s...", key);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        timebar_stream = connect_candle_stream(&timebar_upstream, &symbol, &venue, &key).await;
                    }
                    Err(e) => {
                        warn!("Timebar stream error for {}: {}. Reconnecting in 2s...", key, e);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        timebar_stream = connect_candle_stream(&timebar_upstream, &symbol, &venue, &key).await;
                    }
                }
            }
            msg = tibs_stream.message() => {
                match msg {
                    Ok(Some(m)) => {
                        if let Some(market_data_message::Data::Candle(candle)) = m.data {
                            persist_candle(&pool, &schema, &venue, &key, candle).await;
                        }
                    }
                    Ok(None) => {
                        warn!("Tibs stream ended for {}. Reconnecting in 2s...", key);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        tibs_stream = connect_candle_stream(&tibs_upstream, &symbol, &venue, &key).await;
                    }
                    Err(e) => {
                        warn!("Tibs stream error for {}: {}. Reconnecting in 2s...", key, e);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        tibs_stream = connect_candle_stream(&tibs_upstream, &symbol, &venue, &key).await;
                    }
                }
            }
        }
    }
}

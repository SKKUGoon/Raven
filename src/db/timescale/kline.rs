use crate::config::TimescaleConfig;
use crate::proto::control_client::ControlClient;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::{
    market_data_message, ControlRequest, DataType, MarketDataMessage, MarketDataRequest,
};
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
use super::schema::{ensure_schema_and_kline_table, qualify_table, validate_pg_ident};

#[derive(Clone)]
pub struct TimescaleKlineWorker {
    upstream_url: String,
    schema: String,
    pool: Pool<Postgres>,
    dim_cache: Arc<DimCache>,
}

#[tonic::async_trait]
impl StreamWorker for TimescaleKlineWorker {
    async fn run(&self, key: StreamKey, _tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        let symbol = key.symbol.clone();
        let venue = key.venue.clone().unwrap_or_default();
        let key_str = key.to_string();
        run_kline_persistence_loop(
            self.upstream_url.clone(),
            symbol,
            venue,
            key_str,
            self.schema.clone(),
            self.pool.clone(),
            self.dim_cache.clone(),
        )
        .await;
    }
}

pub type KlinePersistenceService = StreamManager<TimescaleKlineWorker>;

pub async fn new_kline(
    upstream_url: String,
    config: TimescaleConfig,
) -> Result<KlinePersistenceService, sqlx::Error> {
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

    let kline_table = qualify_table(&schema, "bar__kline");
    if let Err(e) = ensure_schema_and_kline_table(&pool, &schema, &kline_table).await {
        warn!(
            "Failed to create/verify kline hypertable (might already exist or not using TimescaleDB): {}",
            e
        );
    }

    let worker = TimescaleKlineWorker {
        upstream_url,
        schema,
        pool,
        dim_cache: Arc::new(DimCache::new()),
    };

    Ok(StreamManager::new(Arc::new(worker), 10_000, false))
}

async fn ensure_upstream_kline_collection(
    upstream_url: &str,
    symbol: &str,
    venue: &str,
    key: &str,
) {
    let mut client = match ControlClient::connect(upstream_url.to_string()).await {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "Failed to connect to upstream Control {} for {}: {}",
                upstream_url, key, e
            );
            return;
        }
    };

    let req = Request::new(ControlRequest {
        symbol: symbol.to_string(),
        venue: venue.to_string(),
        data_type: DataType::Candle as i32,
    });

    if let Err(e) = client.start_collection(req).await {
        warn!(
            "Failed to start upstream kline collection for {}: {}",
            key, e
        );
    }
}

async fn connect_kline_stream(
    upstream_url: &str,
    symbol: &str,
    venue: &str,
    key: &str,
) -> tonic::Streaming<MarketDataMessage> {
    let wildcard = symbol.trim().is_empty() || symbol.trim() == "*";
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
                if e.code() == tonic::Code::FailedPrecondition && !wildcard {
                    ensure_upstream_kline_collection(upstream_url, symbol, venue, key).await;
                }
                warn!(
                    "Failed to subscribe to klines ({} -> {}): {}. Retrying in 2s...",
                    key, upstream_url, e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        }
    }
}

async fn persist_kline(
    dim_cache: &DimCache,
    pool: &Pool<Postgres>,
    schema: &str,
    venue: &str,
    key: &str,
    candle: crate::proto::Candle,
) {
    let time = chrono::DateTime::from_timestamp_millis(candle.timestamp);
    let Some(t) = time else {
        return;
    };

    let table_name = qualify_table(schema, "bar__kline");
    let symbol_id = match dim_cache.resolve_symbol(pool, schema, &candle.symbol).await {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to resolve symbol dimension for {}: {}", key, e);
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
        INSERT INTO {table_name} (time, symbol_id, exchange_id, interval_id, open, high, low, close, volume)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    );

    let result = sqlx::query(&query)
        .bind(t)
        .bind(symbol_id)
        .bind(exchange_id)
        .bind(interval_id)
        .bind(candle.open)
        .bind(candle.high)
        .bind(candle.low)
        .bind(candle.close)
        .bind(candle.volume)
        .execute(pool)
        .await;

    if let Err(e) = result {
        error!("Failed to insert kline for {}: {}", key, e);
    }
}

async fn run_kline_persistence_loop(
    upstream_url: String,
    symbol: String,
    venue: String,
    key: String,
    schema: String,
    pool: Pool<Postgres>,
    dim_cache: Arc<DimCache>,
) {
    info!("Kline persistence task started for {}", key);
    let mut stream = connect_kline_stream(&upstream_url, &symbol, &venue, &key).await;

    loop {
        match stream.message().await {
            Ok(Some(m)) => {
                if let Some(market_data_message::Data::Candle(candle)) = m.data {
                    persist_kline(&dim_cache, &pool, &schema, &venue, &key, candle).await;
                }
            }
            Ok(None) => {
                warn!("Kline stream ended for {}. Reconnecting in 2s...", key);
                tokio::time::sleep(Duration::from_secs(2)).await;
                stream = connect_kline_stream(&upstream_url, &symbol, &venue, &key).await;
            }
            Err(e) => {
                warn!(
                    "Kline stream error for {}: {}. Reconnecting in 2s...",
                    key, e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                stream = connect_kline_stream(&upstream_url, &symbol, &venue, &key).await;
            }
        }
    }
}

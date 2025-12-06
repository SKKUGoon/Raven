use crate::config::TimescaleConfig;
use crate::proto::market_data_client::MarketDataClient;
use crate::proto::{market_data_message, DataType, MarketDataMessage, MarketDataRequest};
use crate::service::{StreamManager, StreamWorker};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::Status;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct TimescaleWorker {
    upstreams: Arc<HashMap<String, String>>,
    default_upstream: String,
    pool: Pool<Postgres>,
}

#[tonic::async_trait]
impl StreamWorker for TimescaleWorker {
    async fn run(&self, key: String, _tx: broadcast::Sender<Result<MarketDataMessage, Status>>) {
        // Parse key (SYMBOL:EXCHANGE or just SYMBOL)
        let (symbol, exchange) = match key.split_once(':') {
            Some((s, e)) => (s.to_string(), e.to_string()),
            None => (key.clone(), String::new()),
        };

        // Select upstream
        let upstream_url = if !exchange.is_empty() {
            self.upstreams.get(&exchange).cloned().unwrap_or_else(|| {
                warn!(
                    "No upstream found for exchange '{}', falling back to default",
                    exchange
                );
                self.default_upstream.clone()
            })
        } else {
            self.default_upstream.clone()
        };

        run_persistence(upstream_url, symbol, exchange, key, self.pool.clone()).await;
    }
}

pub type PersistenceService = StreamManager<TimescaleWorker>;

pub async fn new(
    default_upstream: String,
    upstreams: HashMap<String, String>,
    config: TimescaleConfig,
) -> Result<PersistenceService, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.url)
        .await?;

    // Ensure the table exists (basic migration)
    // In a production env, use proper migrations.
    sqlx::query(
        r#"
        CREATE SCHEMA IF NOT EXISTS data_warehouse;
        
        -- Table for Tick Imbalance Bars
        CREATE TABLE IF NOT EXISTS data_warehouse.bar__tick_imbalance (
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
        SELECT create_hypertable('data_warehouse.bar__tick_imbalance', 'time', if_not_exists => TRUE);

        -- Table for Time Bars
        CREATE TABLE IF NOT EXISTS data_warehouse.bar__time (
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
        SELECT create_hypertable('data_warehouse.bar__time', 'time', if_not_exists => TRUE);
        "#,
    )
    .execute(&pool)
    .await
    .unwrap_or_else(|e| {
        warn!(
            "Failed to create/verify hypertable (might already exist or not using TimescaleDB): {}",
            e
        );
        Default::default()
    });

    let worker = TimescaleWorker {
        upstreams: Arc::new(upstreams),
        default_upstream,
        pool,
    };

    Ok(StreamManager::new(Arc::new(worker), 10000, false))
}

async fn run_persistence(
    upstream_url: String,
    symbol: String,
    exchange: String,
    key: String, // For logging
    pool: Pool<Postgres>,
) {
    let mut client = match MarketDataClient::connect(upstream_url.clone()).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to upstream {}: {}", upstream_url, e);
            return;
        }
    };

    let request = tonic::Request::new(MarketDataRequest {
        symbol: symbol.clone(),
        data_type: DataType::Candle as i32,
    });

    let mut stream = match client.subscribe(request).await {
        Ok(res) => res.into_inner(),
        Err(e) => {
            error!("Failed to subscribe to upstream: {}", e);
            return;
        }
    };

    info!("Bar Persistence task started for {}", key);

    loop {
        match stream.message().await {
            Ok(Some(msg)) => {
                if let Some(market_data_message::Data::Candle(candle)) = msg.data {
                    // Convert timestamp (ms or ns? Proto says int64, usually ms in this system)
                    // Assuming ms based on other parts of the code
                    let time = chrono::DateTime::from_timestamp_millis(candle.timestamp);

                    if let Some(t) = time {
                        let table_name = if candle.interval == "tib" {
                            "data_warehouse.bar__tick_imbalance"
                        } else {
                            "data_warehouse.bar__time"
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
                            .bind(&exchange) // Use the exchange from the key, or potentially from the msg if available
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
                            .execute(&pool)
                            .await;

                        if let Err(e) = result {
                            error!("Failed to insert candle for {}: {}", key, e);
                        }
                    }
                }
            }
            Ok(None) => {
                info!("Stream ended for {}", key);
                break;
            }
            Err(e) => {
                error!("Error receiving message for {}: {}", key, e);
                break;
            }
        }
    }

    info!("Bar Persistence task ended for {}", key);
}

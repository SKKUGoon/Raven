use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tokio::sync::RwLock;

use super::symbol_pair::parse_coin_quote;

#[derive(Default)]
pub struct DimCache {
    coins: RwLock<HashMap<String, i32>>,
    quotes: RwLock<HashMap<String, i32>>,
    exchanges: RwLock<HashMap<String, i32>>,
    intervals: RwLock<HashMap<String, i32>>,
}

impl DimCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn resolve_coin_quote(
        &self,
        pool: &Pool<Postgres>,
        schema: &str,
        value: &str,
    ) -> Result<(i32, i32), sqlx::Error> {
        let (coin, quote) = parse_coin_quote(value).ok_or_else(|| {
            sqlx::Error::Protocol(format!("failed to parse coin/quote from symbol '{value}'"))
        })?;

        let coin_id = resolve_soft_dim_id(
            &self.coins,
            pool,
            schema,
            "dim__coin",
            "coin_id",
            "coin",
            &coin,
        )
        .await?;
        let quote_id = resolve_soft_dim_id(
            &self.quotes,
            pool,
            schema,
            "dim__quote",
            "quote_id",
            "quote",
            &quote,
        )
        .await?;
        Ok((coin_id, quote_id))
    }

    pub async fn resolve_exchange(
        &self,
        pool: &Pool<Postgres>,
        schema: &str,
        value: &str,
    ) -> Result<i32, sqlx::Error> {
        resolve_soft_dim_id(
            &self.exchanges,
            pool,
            schema,
            "dim__exchange",
            "exchange_id",
            "exchange",
            value,
        )
        .await
    }

    pub async fn resolve_interval(
        &self,
        pool: &Pool<Postgres>,
        schema: &str,
        value: &str,
    ) -> Result<i32, sqlx::Error> {
        resolve_soft_dim_id(
            &self.intervals,
            pool,
            schema,
            "dim__interval",
            "interval_id",
            "interval",
            value,
        )
        .await
    }
}

async fn resolve_soft_dim_id(
    cache: &RwLock<HashMap<String, i32>>,
    pool: &Pool<Postgres>,
    schema: &str,
    table: &str,
    id_col: &str,
    value_col: &str,
    value: &str,
) -> Result<i32, sqlx::Error> {
    if let Some(id) = cache.read().await.get(value).copied() {
        return Ok(id);
    }

    let query = format!(
        r#"
        INSERT INTO {schema}.{table} ({value_col}, is_deleted, deleted_date)
        VALUES ($1, FALSE, NULL)
        ON CONFLICT ({value_col}) DO UPDATE
        SET is_deleted = FALSE, deleted_date = NULL
        RETURNING {id_col}
        "#
    );

    let id: i32 = sqlx::query_scalar(&query).bind(value).fetch_one(pool).await?;
    cache.write().await.insert(value.to_string(), id);
    Ok(id)
}

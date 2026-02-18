use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct DimCache {
    symbols: RwLock<HashMap<String, i32>>,
    exchanges: RwLock<HashMap<String, i32>>,
    intervals: RwLock<HashMap<String, i32>>,
}

impl DimCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn resolve_symbol(
        &self,
        pool: &Pool<Postgres>,
        schema: &str,
        value: &str,
    ) -> Result<i32, sqlx::Error> {
        resolve_dim_id(&self.symbols, pool, schema, "dim_symbol", "symbol_id", "symbol", value).await
    }

    pub async fn resolve_exchange(
        &self,
        pool: &Pool<Postgres>,
        schema: &str,
        value: &str,
    ) -> Result<i32, sqlx::Error> {
        resolve_dim_id(
            &self.exchanges,
            pool,
            schema,
            "dim_exchange",
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
        resolve_dim_id(
            &self.intervals,
            pool,
            schema,
            "dim_interval",
            "interval_id",
            "interval",
            value,
        )
        .await
    }
}

async fn resolve_dim_id(
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

    let (extra_cols, extra_vals, on_conflict_set) = if table == "dim_symbol" {
        (
            ", is_deleted, deleted_date",
            ", FALSE, NULL",
            format!("{value_col} = EXCLUDED.{value_col}, is_deleted = FALSE, deleted_date = NULL"),
        )
    } else {
        (
            "",
            "",
            format!("{value_col} = EXCLUDED.{value_col}"),
        )
    };

    let query = format!(
        r#"
        INSERT INTO {schema}.{table} ({value_col}{extra_cols})
        VALUES ($1{extra_vals})
        ON CONFLICT ({value_col}) DO UPDATE SET {on_conflict_set}
        RETURNING {id_col}
        "#
    );

    let id: i32 = sqlx::query_scalar(&query).bind(value).fetch_one(pool).await?;
    cache.write().await.insert(value.to_string(), id);
    Ok(id)
}

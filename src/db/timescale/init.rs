use crate::config::TimescaleConfig;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::collections::BTreeSet;
use tracing::info;

use super::schema::{ensure_schema_and_dimensions, validate_pg_ident};
use super::symbol_pair::parse_coin_quote;

#[derive(Debug, Clone, Default)]
pub struct RavenInitSeed {
    pub symbols: Vec<String>,
    pub exchanges: Vec<String>,
    pub intervals: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct SeedStats {
    added: usize,
    existing: usize,
    reactivated: usize,
}

pub async fn run_raven_init(
    config: &TimescaleConfig,
    mut seed: RavenInitSeed,
) -> Result<(), sqlx::Error> {
    seed.symbols.sort();
    seed.symbols.dedup();
    seed.exchanges.sort();
    seed.exchanges.dedup();
    seed.intervals.sort();
    seed.intervals.dedup();

    let schema = validate_pg_ident(&config.schema)
        .unwrap_or("mart")
        .to_string();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.url)
        .await?;

    ensure_schema_and_dimensions(&pool, &schema).await?;

    let (coins, quotes) = split_coin_quote(&seed.symbols);
    let coin_stats =
        seed_simple_dimensions(&pool, &schema, "dim__coin", "coin_id", "coin", &coins).await?;
    log_stats("dim__coin", coin_stats);
    let quote_stats =
        seed_simple_dimensions(&pool, &schema, "dim__quote", "quote_id", "quote", &quotes).await?;
    log_stats("dim__quote", quote_stats);
    let exch = seed_simple_dimensions(
        &pool,
        &schema,
        "dim__exchange",
        "exchange_id",
        "exchange",
        &seed.exchanges,
    )
    .await?;
    log_stats("dim__exchange", exch);
    let intr = seed_simple_dimensions(
        &pool,
        &schema,
        "dim__interval",
        "interval_id",
        "interval",
        &seed.intervals,
    )
    .await?;
    log_stats("dim__interval", intr);

    Ok(())
}

fn log_stats(name: &str, stats: SeedStats) {
    if stats.added == 0 && stats.reactivated == 0 {
        info!(
            "{name}: data already populated (existing={}, new=0, reactivated=0)",
            stats.existing
        );
    } else {
        info!(
            "{name}: initialization complete (new={}, reactivated={}, existing={})",
            stats.added, stats.reactivated, stats.existing
        );
    }
}

async fn seed_simple_dimensions(
    pool: &Pool<Postgres>,
    schema: &str,
    table: &str,
    id_col: &str,
    value_col: &str,
    values: &[String],
) -> Result<SeedStats, sqlx::Error> {
    let mut stats = SeedStats::default();
    let active_values: BTreeSet<String> = values
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();

    for value in &active_values {
        if insert_soft_dim(pool, schema, table, id_col, value_col, value).await? {
            stats.added += 1;
            info!("DIMENSION_ADDED {table} {value_col}={value}");
        } else {
            let was_deleted: bool = sqlx::query_scalar(&format!(
                "SELECT is_deleted FROM {schema}.{table} WHERE {value_col} = $1"
            ))
            .bind(value)
            .fetch_one(pool)
            .await?;
            if was_deleted {
                stats.reactivated += 1;
                info!("DIMENSION_REACTIVATED {table} {value_col}={value}");
            } else {
                stats.existing += 1;
            }
        }
    }

    if !active_values.is_empty() {
        let existing_active: Vec<String> = sqlx::query_scalar(&format!(
            r#"
            SELECT {value_col}
            FROM {schema}.{table}
            WHERE is_deleted = FALSE
            "#
        ))
        .fetch_all(pool)
        .await?;
        for value in existing_active {
            if active_values.contains(&value) {
                continue;
            }
            sqlx::query(&format!(
                "UPDATE {schema}.{table} SET is_deleted = TRUE, deleted_date = NOW() WHERE {value_col} = $1"
            ))
            .bind(&value)
            .execute(pool)
            .await?;
            info!("DIMENSION_SOFT_DELETED {table} {value_col}={value}");
        }
    }

    Ok(stats)
}

async fn insert_soft_dim(
    pool: &Pool<Postgres>,
    schema: &str,
    table: &str,
    id_col: &str,
    value_col: &str,
    value: &str,
) -> Result<bool, sqlx::Error> {
    let query = format!(
        r#"
        INSERT INTO {schema}.{table} ({value_col}, is_deleted, deleted_date)
        VALUES ($1, FALSE, NULL)
        ON CONFLICT ({value_col}) DO UPDATE
        SET is_deleted = FALSE, deleted_date = NULL
        RETURNING {id_col}
        "#
    );
    let inserted = sqlx::query_scalar::<_, i32>(&query)
        .bind(value)
        .fetch_optional(pool)
        .await?;
    Ok(inserted.is_some())
}

fn split_coin_quote(symbols: &[String]) -> (Vec<String>, Vec<String>) {
    let mut coins = BTreeSet::new();
    let mut quotes = BTreeSet::new();
    for symbol in symbols {
        if let Some((coin, quote)) = parse_coin_quote(symbol) {
            coins.insert(coin);
            quotes.insert(quote);
        }
    }
    (coins.into_iter().collect(), quotes.into_iter().collect())
}

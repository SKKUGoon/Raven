use crate::config::TimescaleConfig;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use tracing::info;

use super::schema::{ensure_schema_and_dimensions, validate_pg_ident};

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

    let exch = seed_exchange_dimensions(&pool, &schema, &seed.exchanges).await?;
    log_stats("dim_exchange", exch);
    let intr = seed_interval_dimensions(&pool, &schema, &seed.intervals).await?;
    log_stats("dim_interval", intr);
    let sym = seed_symbol_dimensions(&pool, &schema, &seed.symbols).await?;
    log_stats("dim_symbol", sym);

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

async fn seed_exchange_dimensions(
    pool: &Pool<Postgres>,
    schema: &str,
    exchanges: &[String],
) -> Result<SeedStats, sqlx::Error> {
    let mut stats = SeedStats::default();
    for exchange in exchanges {
        if insert_simple_dim(pool, schema, "dim_exchange", "exchange_id", "exchange", exchange).await?
        {
            stats.added += 1;
            info!("DIMENSION_ADDED dim_exchange exchange={exchange}");
        } else {
            stats.existing += 1;
        }
    }
    Ok(stats)
}

async fn seed_interval_dimensions(
    pool: &Pool<Postgres>,
    schema: &str,
    intervals: &[String],
) -> Result<SeedStats, sqlx::Error> {
    let mut stats = SeedStats::default();
    for interval in intervals {
        if insert_simple_dim(pool, schema, "dim_interval", "interval_id", "interval", interval).await?
        {
            stats.added += 1;
            info!("DIMENSION_ADDED dim_interval interval={interval}");
        } else {
            stats.existing += 1;
        }
    }
    Ok(stats)
}

async fn seed_symbol_dimensions(
    pool: &Pool<Postgres>,
    schema: &str,
    symbols: &[String],
) -> Result<SeedStats, sqlx::Error> {
    let mut stats = SeedStats::default();
    for symbol in symbols {
        let table = format!("{schema}.dim_symbol");
        let inserted = sqlx::query_scalar::<_, i32>(&format!(
            r#"
            INSERT INTO {table} (symbol, is_deleted, deleted_date)
            VALUES ($1, FALSE, NULL)
            ON CONFLICT (symbol) DO NOTHING
            RETURNING symbol_id
            "#
        ))
        .bind(symbol)
        .fetch_optional(pool)
        .await?;

        if inserted.is_some() {
            stats.added += 1;
            info!("DIMENSION_ADDED dim_symbol symbol={symbol}");
            continue;
        }

        let was_deleted = sqlx::query_scalar::<_, bool>(&format!(
            "SELECT is_deleted FROM {table} WHERE symbol = $1"
        ))
        .bind(symbol)
        .fetch_one(pool)
        .await?;

        if was_deleted {
            sqlx::query(&format!(
                "UPDATE {table} SET is_deleted = FALSE, deleted_date = NULL WHERE symbol = $1"
            ))
            .bind(symbol)
            .execute(pool)
            .await?;
            stats.reactivated += 1;
            info!("DIMENSION_REACTIVATED dim_symbol symbol={symbol}");
        } else {
            stats.existing += 1;
        }
    }

    if !symbols.is_empty() {
        let table = format!("{schema}.dim_symbol");
        let active_symbols: Vec<String> = symbols.to_vec();
        let discontinued: Vec<String> = sqlx::query_scalar(&format!(
            r#"
            SELECT symbol
            FROM {table}
            WHERE is_deleted = FALSE
              AND NOT (symbol = ANY($1))
            "#
        ))
        .bind(&active_symbols)
        .fetch_all(pool)
        .await?;

        for symbol in discontinued {
            sqlx::query(&format!(
                "UPDATE {table} SET is_deleted = TRUE, deleted_date = NOW() WHERE symbol = $1"
            ))
            .bind(&symbol)
            .execute(pool)
            .await?;
            info!("DIMENSION_SOFT_DELETED dim_symbol symbol={symbol}");
        }
    }

    Ok(stats)
}

async fn insert_simple_dim(
    pool: &Pool<Postgres>,
    schema: &str,
    table: &str,
    id_col: &str,
    value_col: &str,
    value: &str,
) -> Result<bool, sqlx::Error> {
    let query = format!(
        r#"
        INSERT INTO {schema}.{table} ({value_col})
        VALUES ($1)
        ON CONFLICT ({value_col}) DO NOTHING
        RETURNING {id_col}
        "#
    );
    let inserted = sqlx::query_scalar::<_, i32>(&query)
        .bind(value)
        .fetch_optional(pool)
        .await?;
    Ok(inserted.is_some())
}

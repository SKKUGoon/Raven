use sqlx::{Pool, Postgres};

pub(super) fn validate_pg_ident(s: &str) -> Option<&str> {
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

pub(super) fn qualify_table(schema: &str, table: &str) -> String {
    format!("{schema}.{table}")
}

async fn ensure_dimension_tables(pool: &Pool<Postgres>, schema: &str) -> Result<(), sqlx::Error> {
    let dim_coin = qualify_table(schema, "dim__coin");
    let dim_quote = qualify_table(schema, "dim__quote");
    let dim_exchange = qualify_table(schema, "dim__exchange");
    let dim_interval = qualify_table(schema, "dim__interval");

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {dim_coin} (
            coin_id      SERIAL PRIMARY KEY,
            coin        TEXT NOT NULL,
            is_deleted  BOOLEAN NOT NULL DEFAULT FALSE,
            deleted_date TIMESTAMPTZ NULL,
            UNIQUE (coin)
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {dim_quote} (
            quote_id     SERIAL PRIMARY KEY,
            quote       TEXT NOT NULL,
            is_deleted  BOOLEAN NOT NULL DEFAULT FALSE,
            deleted_date TIMESTAMPTZ NULL,
            UNIQUE (quote)
        );
        "#
    ))
    .execute(pool)
    .await?;

    // Backward-compatible column adds for existing databases.
    sqlx::query(&format!(
        "ALTER TABLE {dim_coin} ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {dim_coin} ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {dim_quote} ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {dim_quote} ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {dim_exchange} (
            exchange_id SERIAL PRIMARY KEY,
            exchange    TEXT NOT NULL UNIQUE,
            is_deleted  BOOLEAN NOT NULL DEFAULT FALSE,
            deleted_date TIMESTAMPTZ NULL
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {dim_interval} (
            interval_id SERIAL PRIMARY KEY,
            interval    TEXT NOT NULL UNIQUE,
            is_deleted  BOOLEAN NOT NULL DEFAULT FALSE,
            deleted_date TIMESTAMPTZ NULL
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "ALTER TABLE {dim_exchange} ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {dim_exchange} ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {dim_interval} ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {dim_interval} ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;"
    ))
    .execute(pool)
    .await?;

    Ok(())
}

pub(super) async fn ensure_schema_and_dimensions(
    pool: &Pool<Postgres>,
    schema: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema};"))
        .execute(pool)
        .await?;
    ensure_dimension_tables(pool, schema).await
}

pub(super) async fn ensure_schema_and_tables(
    pool: &Pool<Postgres>,
    schema: &str,
    _tib_table: &str,
    _vib_table: &str,
    _vpin_table: &str,
) -> Result<(), sqlx::Error> {
    // Postgres drivers generally disallow multiple statements in a single prepared statement,
    // so keep this strictly one statement per execute().
    ensure_schema_and_dimensions(pool, schema).await?;

    let fact_tib = qualify_table(schema, "fact__tick_imbalance");
    let fact_vib = qualify_table(schema, "fact__volume_imbalance");
    let fact_vpin = qualify_table(schema, "fact__vpin");

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {fact_tib} (
            time        TIMESTAMPTZ NOT NULL,
            coin_id     INTEGER NOT NULL REFERENCES {schema}.dim__coin(coin_id),
            quote_id    INTEGER NOT NULL REFERENCES {schema}.dim__quote(quote_id),
            exchange_id INTEGER NOT NULL REFERENCES {schema}.dim__exchange(exchange_id),
            interval_id INTEGER NOT NULL REFERENCES {schema}.dim__interval(interval_id),
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            buy_ticks   BIGINT NOT NULL DEFAULT 0,
            sell_ticks  BIGINT NOT NULL DEFAULT 0,
            total_ticks BIGINT NOT NULL DEFAULT 0,
            theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
            UNIQUE (time, coin_id, quote_id, exchange_id, interval_id)
        );
        "#
    ))
    .execute(pool)
    .await?;

    // If TimescaleDB isn't installed/enabled, this will error; that's okay.
    // (Callers treat this as best-effort setup.)
    sqlx::query(&format!(
        "SELECT create_hypertable('{fact_tib}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {schema}_fact_tib_cid_qid_eid_iid_time_idx ON {fact_tib} (coin_id, quote_id, exchange_id, interval_id, time DESC);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {fact_vib} (
            time        TIMESTAMPTZ NOT NULL,
            coin_id     INTEGER NOT NULL REFERENCES {schema}.dim__coin(coin_id),
            quote_id    INTEGER NOT NULL REFERENCES {schema}.dim__quote(quote_id),
            exchange_id INTEGER NOT NULL REFERENCES {schema}.dim__exchange(exchange_id),
            interval_id INTEGER NOT NULL REFERENCES {schema}.dim__interval(interval_id),
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            buy_ticks   BIGINT NOT NULL DEFAULT 0,
            sell_ticks  BIGINT NOT NULL DEFAULT 0,
            total_ticks BIGINT NOT NULL DEFAULT 0,
            theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
            UNIQUE (time, coin_id, quote_id, exchange_id, interval_id)
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "SELECT create_hypertable('{fact_vib}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {schema}_fact_vib_cid_qid_eid_iid_time_idx ON {fact_vib} (coin_id, quote_id, exchange_id, interval_id, time DESC);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {fact_vpin} (
            time        TIMESTAMPTZ NOT NULL,
            coin_id     INTEGER NOT NULL REFERENCES {schema}.dim__coin(coin_id),
            quote_id    INTEGER NOT NULL REFERENCES {schema}.dim__quote(quote_id),
            exchange_id INTEGER NOT NULL REFERENCES {schema}.dim__exchange(exchange_id),
            interval_id INTEGER NOT NULL REFERENCES {schema}.dim__interval(interval_id),
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            buy_ticks   BIGINT NOT NULL DEFAULT 0,
            sell_ticks  BIGINT NOT NULL DEFAULT 0,
            total_ticks BIGINT NOT NULL DEFAULT 0,
            theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
            UNIQUE (time, coin_id, quote_id, exchange_id, interval_id)
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "SELECT create_hypertable('{fact_vpin}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {schema}_fact_vpin_cid_qid_eid_iid_time_idx ON {fact_vpin} (coin_id, quote_id, exchange_id, interval_id, time DESC);"
    ))
    .execute(pool)
    .await?;

    Ok(())
}

pub(super) async fn ensure_schema_and_kline_table(
    pool: &Pool<Postgres>,
    schema: &str,
    _table: &str,
) -> Result<(), sqlx::Error> {
    ensure_schema_and_dimensions(pool, schema).await?;
    let fact_kline = qualify_table(schema, "fact__kline");

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {fact_kline} (
            time        TIMESTAMPTZ NOT NULL,
            coin_id     INTEGER NOT NULL REFERENCES {schema}.dim__coin(coin_id),
            quote_id    INTEGER NOT NULL REFERENCES {schema}.dim__quote(quote_id),
            exchange_id INTEGER NOT NULL REFERENCES {schema}.dim__exchange(exchange_id),
            interval_id INTEGER NOT NULL REFERENCES {schema}.dim__interval(interval_id),
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            UNIQUE (time, coin_id, quote_id, exchange_id, interval_id)
        );
        "#
    ))
    .execute(pool)
    .await?;

    // If TimescaleDB isn't installed/enabled, this will error; that's okay.
    sqlx::query(&format!(
        "SELECT create_hypertable('{fact_kline}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    Ok(())
}

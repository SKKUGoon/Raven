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
    let dim_symbol = qualify_table(schema, "dim_symbol");
    let dim_exchange = qualify_table(schema, "dim_exchange");
    let dim_interval = qualify_table(schema, "dim_interval");

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {dim_symbol} (
            symbol_id   SERIAL PRIMARY KEY,
            symbol      TEXT NOT NULL UNIQUE,
            is_deleted  BOOLEAN NOT NULL DEFAULT FALSE,
            deleted_date TIMESTAMPTZ NULL
        );
        "#
    ))
    .execute(pool)
    .await?;

    // Backward-compatible column adds for existing databases.
    sqlx::query(&format!(
        "ALTER TABLE {dim_symbol} ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "ALTER TABLE {dim_symbol} ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {dim_exchange} (
            exchange_id SERIAL PRIMARY KEY,
            exchange    TEXT NOT NULL UNIQUE
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {dim_interval} (
            interval_id SERIAL PRIMARY KEY,
            interval    TEXT NOT NULL UNIQUE
        );
        "#
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
    tib_table: &str,
    vib_table: &str,
    vpin_table: &str,
) -> Result<(), sqlx::Error> {
    // Postgres drivers generally disallow multiple statements in a single prepared statement,
    // so keep this strictly one statement per execute().
    ensure_schema_and_dimensions(pool, schema).await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {tib_table} (
            time        TIMESTAMPTZ NOT NULL,
            symbol_id   INTEGER NOT NULL REFERENCES {schema}.dim_symbol(symbol_id),
            exchange_id INTEGER NOT NULL REFERENCES {schema}.dim_exchange(exchange_id),
            interval_id INTEGER NOT NULL REFERENCES {schema}.dim_interval(interval_id),
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            buy_ticks   BIGINT NOT NULL DEFAULT 0,
            sell_ticks  BIGINT NOT NULL DEFAULT 0,
            total_ticks BIGINT NOT NULL DEFAULT 0,
            theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
            UNIQUE (time, symbol_id, exchange_id, interval_id)
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
        "CREATE INDEX IF NOT EXISTS {schema}_tib_sid_eid_iid_time_idx ON {tib_table} (symbol_id, exchange_id, interval_id, time DESC);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {vib_table} (
            time        TIMESTAMPTZ NOT NULL,
            symbol_id   INTEGER NOT NULL REFERENCES {schema}.dim_symbol(symbol_id),
            exchange_id INTEGER NOT NULL REFERENCES {schema}.dim_exchange(exchange_id),
            interval_id INTEGER NOT NULL REFERENCES {schema}.dim_interval(interval_id),
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            buy_ticks   BIGINT NOT NULL DEFAULT 0,
            sell_ticks  BIGINT NOT NULL DEFAULT 0,
            total_ticks BIGINT NOT NULL DEFAULT 0,
            theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
            UNIQUE (time, symbol_id, exchange_id, interval_id)
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "SELECT create_hypertable('{vib_table}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {schema}_vib_sid_eid_iid_time_idx ON {vib_table} (symbol_id, exchange_id, interval_id, time DESC);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {vpin_table} (
            time        TIMESTAMPTZ NOT NULL,
            symbol_id   INTEGER NOT NULL REFERENCES {schema}.dim_symbol(symbol_id),
            exchange_id INTEGER NOT NULL REFERENCES {schema}.dim_exchange(exchange_id),
            interval_id INTEGER NOT NULL REFERENCES {schema}.dim_interval(interval_id),
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            buy_ticks   BIGINT NOT NULL DEFAULT 0,
            sell_ticks  BIGINT NOT NULL DEFAULT 0,
            total_ticks BIGINT NOT NULL DEFAULT 0,
            theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
            UNIQUE (time, symbol_id, exchange_id, interval_id)
        );
        "#
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "SELECT create_hypertable('{vpin_table}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {schema}_vpin_sid_eid_iid_time_idx ON {vpin_table} (symbol_id, exchange_id, interval_id, time DESC);"
    ))
    .execute(pool)
    .await?;

    Ok(())
}

pub(super) async fn ensure_schema_and_kline_table(
    pool: &Pool<Postgres>,
    schema: &str,
    table: &str,
) -> Result<(), sqlx::Error> {
    ensure_schema_and_dimensions(pool, schema).await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {table} (
            time        TIMESTAMPTZ NOT NULL,
            symbol_id   INTEGER NOT NULL REFERENCES {schema}.dim_symbol(symbol_id),
            exchange_id INTEGER NOT NULL REFERENCES {schema}.dim_exchange(exchange_id),
            interval_id INTEGER NOT NULL REFERENCES {schema}.dim_interval(interval_id),
            open        DOUBLE PRECISION NOT NULL,
            high        DOUBLE PRECISION NOT NULL,
            low         DOUBLE PRECISION NOT NULL,
            close       DOUBLE PRECISION NOT NULL,
            volume      DOUBLE PRECISION NOT NULL,
            UNIQUE (time, symbol_id, exchange_id, interval_id)
        );
        "#
    ))
    .execute(pool)
    .await?;

    // If TimescaleDB isn't installed/enabled, this will error; that's okay.
    sqlx::query(&format!(
        "SELECT create_hypertable('{table}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    Ok(())
}

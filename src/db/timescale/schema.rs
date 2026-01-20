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

pub(super) async fn ensure_schema_and_tables(
    pool: &Pool<Postgres>,
    schema: &str,
    time_table: &str,
    tib_table: &str,
    vib_table: &str,
    vpin_table: &str,
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
        CREATE TABLE IF NOT EXISTS {vib_table} (
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
        "SELECT create_hypertable('{vib_table}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {vpin_table} (
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
        "SELECT create_hypertable('{vpin_table}', 'time', if_not_exists => TRUE);"
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

pub(super) async fn ensure_schema_and_kline_table(
    pool: &Pool<Postgres>,
    schema: &str,
    table: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema};"))
        .execute(pool)
        .await?;

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {table} (
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
    sqlx::query(&format!(
        "SELECT create_hypertable('{table}', 'time', if_not_exists => TRUE);"
    ))
    .execute(pool)
    .await?;

    Ok(())
}

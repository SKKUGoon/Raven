use sqlx::{Pool, Postgres};
const TIMESCALE_SCHEMA_SQL: &str = include_str!("../../../sql/timescale_schema.sql");

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

async fn ensure_dimension_tables(pool: &Pool<Postgres>, schema: &str) -> Result<(), sqlx::Error> {
    execute_timescale_schema_script(pool, schema).await
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
    ensure_schema_and_dimensions(pool, schema).await
}

pub(super) async fn ensure_schema_and_kline_table(
    pool: &Pool<Postgres>,
    schema: &str,
    _table: &str,
) -> Result<(), sqlx::Error> {
    ensure_schema_and_dimensions(pool, schema).await
}

async fn execute_timescale_schema_script(
    pool: &Pool<Postgres>,
    schema: &str,
) -> Result<(), sqlx::Error> {
    let rendered = TIMESCALE_SCHEMA_SQL.replace("__SCHEMA__", schema);
    for raw_stmt in rendered.split(';') {
        let stmt = raw_stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        let sql = format!("{stmt};");
        sqlx::query(&sql).execute(pool).await?;
    }
    Ok(())
}

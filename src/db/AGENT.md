# src/db/ – Database writers

Persistence layer for **InfluxDB v2** (ticks, orderbook snapshots) and **TimescaleDB** (bars, klines). Used by the binaries in `src/bin/persist/`.

## Layout

| Module | Role |
|--------|------|
| `influx` | InfluxDB client and write logic; trade and orderbook points. |
| `timescale` | PostgreSQL/TimescaleDB: schema, bar tables, kline table. |

## InfluxDB

- **Config**: `config::InfluxConfig` (url, org, bucket, token).
- **Worker / persistence**: `influx/worker.rs`, `influx/persistence.rs` – batch and write points; handle retries and errors.
- **Schema**: schema-on-write; document measurement names and tags in code or comments.

## TimescaleDB

- **Config**: `config::TimescaleConfig` (connection, schema name, e.g. `warehouse`).
- **Schema**: `timescale/schema.rs` – table/schema creation; `create_hypertable` for time-series tables.
- **Tables**: `bar__time`, `bar__tick_imbalance`, `bar__volume_imbalance`, `bar__kline` (see `timescale/bars.rs`, `timescale/kline.rs`).
- **Reference DDL**: `sql/create_table_bar__*.sql`; keep in sync with code when changing schema.

## Conventions

- **Errors**: use `thiserror`; retry transient failures; log and surface permanent failures.
- **Batching**: prefer batching writes for throughput; respect Influx/Timescale limits.
- **Schema changes**: update both Rust code and `sql/` DDL; consider migrations if needed for production.

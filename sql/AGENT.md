# sql/ – Reference DDL for TimescaleDB

Reference SQL for bar and kline tables used by **bar_persistence** and **kline_persistence**. The actual schema may be created at runtime by the persistence binaries (best-effort); this directory is the source of truth for DDL.

## Files

| File | Table | Used by |
|------|--------|--------|
| `create_table_bar__tick_imbalance.sql` | Star schema base + tick-imbalance fact | bar_persistence |

Other fact tables (`bar__volume_imbalance`, `bar__vpin`, `bar__kline`) are created by the persistence binaries at runtime from `src/db/timescale/schema.rs`.

## Conventions

- **TimescaleDB**: use `create_hypertable(...)` for time-partitioned tables; the extension must be installed in the target database.
- **Schema**: default schema name is `mart` (configurable via timescale config in application).
- **Model**: `mart` is warehouse + mart with star schema dimensions (`dim_symbol`, `dim_exchange`, `dim_interval`) and fact hypertables (`bar__*`) referencing dimension IDs via NOT NULL foreign keys.
- When changing table shape or columns, update both the Rust writer in `src/db/timescale/` and the corresponding `.sql` here so migrations or manual setup stay in sync.
- If SQL/runtime DB prerequisites change (extensions, schema assumptions, required services), update `src/bin/persist/dependency_check.rs` so `raven_init` preflight reports missing dependencies correctly.

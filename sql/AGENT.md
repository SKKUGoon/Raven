# sql/ – Reference DDL for TimescaleDB

Reference SQL for bar and kline tables used by **bar_persistence** and **kline_persistence**. The actual schema may be created at runtime by the persistence binaries (best-effort); this directory is the source of truth for DDL.

## Files

| File | Table | Used by |
|------|--------|--------|
| `create_table_bar__time.sql` | Time bar table (e.g. `warehouse.bar__time`) | bar_persistence |
| `create_table_bar__tick_imbalance.sql` | Tick imbalance bar table | bar_persistence |

Other bar/kline tables (e.g. volume imbalance, kline) may be created by the persistence binaries at runtime; add reference DDL here when introduced.

## Conventions

- **TimescaleDB**: use `create_hypertable(...)` for time-partitioned tables; the extension must be installed in the target database.
- **Schema**: default schema name is `warehouse` (configurable via timescale config in application).
- When changing table shape or columns, update both the Rust writer in `src/db/timescale/` and the corresponding `.sql` here so migrations or manual setup stay in sync.

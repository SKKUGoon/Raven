# sql/ – Timescale schema + migrations

TimescaleDB schema source-of-truth SQL is `sql/timescale_schema.sql`. Rust (`src/db/timescale/schema.rs`) loads this file and executes it with schema placeholder substitution.

## Conventions

- **TimescaleDB**: use `create_hypertable(...)` for time-partitioned tables; the extension must be installed in the target database.
- **Schema**: default schema name is `mart` (configurable via timescale config in application).
- **Model**: `mart` is warehouse + mart with star schema dimensions (`dim__coin`, `dim__quote`, `dim__exchange`, `dim__interval`) and fact hypertables (`fact__*`) referencing dimension IDs via NOT NULL foreign keys.
- When changing table shape or columns, update `sql/timescale_schema.sql`; Rust execution code should stay thin.
- If SQL/runtime DB prerequisites change (extensions, schema assumptions, required services), update `src/bin/persist/dependency_check.rs` so `raven_init` preflight reports missing dependencies correctly.

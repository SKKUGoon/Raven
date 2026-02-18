# sql/ – SQL utilities / migrations

TimescaleDB runtime schema is defined in Rust (`src/db/timescale/schema.rs`) and executed by persistence binaries. This directory is reserved for optional ad-hoc SQL utilities or migration scripts.

## Conventions

- **TimescaleDB**: use `create_hypertable(...)` for time-partitioned tables; the extension must be installed in the target database.
- **Schema**: default schema name is `mart` (configurable via timescale config in application).
- **Model**: `mart` is warehouse + mart with star schema dimensions (`dim__coin`, `dim__quote`, `dim__exchange`, `dim__interval`) and fact hypertables (`fact__*`) referencing dimension IDs via NOT NULL foreign keys.
- When changing table shape or columns, update Rust schema code in `src/db/timescale/` (single source of truth).
- If SQL/runtime DB prerequisites change (extensions, schema assumptions, required services), update `src/bin/persist/dependency_check.rs` so `raven_init` preflight reports missing dependencies correctly.

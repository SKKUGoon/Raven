# src/telemetry/ – Metrics and observability

Prometheus metrics for sources (exchange), persistence, and bar emission. Each service exposes HTTP metrics on **gRPC port + 1000** (see `service::RavenService`).

## Layout

| Module | Purpose |
|--------|--------|
| `mod.rs` | Metric registration and common labels. |
| `binance` | Metrics for Binance WS (messages, errors, latency). |
| `persistence` | Metrics for tick/bar persistence (writes, errors). |
| `bars` | Metrics for bar/feature emission (count, latency). |

## Conventions

- Use `prometheus` crate; prefer counters and histograms with clear names and labels (venue, symbol, data_type as appropriate).
- Do not create new global registries in telemetry; use the same registry the server exposes.
- High-cardinality labels (e.g. per-symbol) can be expensive; use sparingly or aggregate.
- Metrics endpoints bind on `service_port + 1000`; when service ports change, also sync `src/bin/persist/dependency_check.rs` so `raven_init` preflight tracks the updated port-bearing services.

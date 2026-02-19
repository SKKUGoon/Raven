# src/utils/ – Shared utilities

Common helpers used across the crate: gRPC clients, process management, retry, tree, service registry, status.

## Modules

| Module | Purpose |
|--------|--------|
| `grpc` | gRPC client helpers (e.g. connect to Control or MarketData). |
| `retry` | Retry with backoff for transient failures. |
| `process` | Process spawn/kill/lifecycle (used by ravenctl for service processes). |
| `tree` | Tree structures (e.g. for graph or status display). |
| `service_registry` | Registry of service names and ports (from config or static). |
| `status` | Status or health check helpers. |

## Conventions

- Keep utils generic and side-effect-free where possible; I/O (process, gRPC) in dedicated modules.
- **Process**: ravenctl uses these to start bins and track PIDs; ensure behavior is consistent on the target OS (macOS/Linux).
- **Retry**: use for WS reconnect, DB write retries, and gRPC calls that may transiently fail.
- Keep `service_registry` aligned with `src/bin/persist/dependency_check.rs` whenever port-bearing services are added/removed/renumbered, so `raven_init` preflight checks are accurate.

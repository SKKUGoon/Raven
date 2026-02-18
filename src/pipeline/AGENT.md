# src/pipeline/ – Pipeline topology and rendering

Definition and rendering of the **data pipeline**: which services exist and how they connect (sources → feature makers → persistence). Used by ravenctl for plan and graph.

## Layout

| Module | Purpose |
|--------|--------|
| `spec` | Pipeline spec: services, edges, data types (TRADE, ORDERBOOK, CANDLE). |
| `render` | Render pipeline to ASCII or DOT (for `ravenctl graph`). |

## Conventions

- The spec should reflect the actual topology: collectors, feature services, persistence; edges represent “subscribes to” or “produces data type”.
- When adding a new service or data flow, update the pipeline spec so `ravenctl plan` and `ravenctl graph` stay accurate.
- Rendering is read-only; no side effects.

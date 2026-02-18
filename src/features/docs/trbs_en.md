# Tick Run Bars (TRBS)

## Overview

Tick Run Bars (TRBS) are another information-driven bar sampling method from López de Prado (2018). While TIBS measure cumulative *imbalance* (net buys minus sells), TRBS measure the longest *run* — the maximum consecutive sequence of same-direction ticks within a bar. Bars close when the longest run exceeds a dynamically adjusted threshold, indicating unusual sequential order flow in one direction.

## Why Use TRBS

- **Detects sequential order flow**: Captures patterns where many consecutive trades go in the same direction, which often signals algorithmic or institutional execution.
- **Complementary to TIBS**: TIBS detect net imbalance; TRBS detect persistence of direction. A market can have balanced net buys/sells but still show suspicious runs.
- **Adaptive threshold**: Like TIBS, the threshold adjusts over time via EWMA, adapting to the current market regime.

## Mathematical Foundation

### Tick Direction

Each trade is classified:

```
b_t = Buy   if trade is buyer-initiated
b_t = Sell   if trade is seller-initiated
```

### Current Run Tracking

A "run" is a consecutive sequence of ticks with the same direction. On each tick:

```
if b_t == run_sign:
    current_run += 1
else:
    θ = max(θ, current_run)    // record the completed run length
    run_sign = b_t              // start new run
    current_run = 1
```

### Theta (Best Run)

`θ` is the longest run observed so far within the current bar:

```
θ = max(all completed run lengths in this bar)
```

### Bar Closing Condition

A bar closes when:

```
θ > E[T] × max(E[p_buy], 1 - E[p_buy])
```

where:
- `E[T]` = expected number of ticks per bar (EWMA of historical bar sizes)
- `E[p_buy]` = expected buy probability (EWMA of observed buy ratios)
- `max(p, 1-p)` = the probability of the dominant direction

The intuition: in a balanced market (`p_buy ≈ 0.5`), the expected longest run in T ticks grows with T. The threshold scales with both the bar size and the directional bias.

### EWMA Updates (After Each Bar Close)

```
E[p_buy] ← α_prob × p_buy_observed + (1 - α_prob) × E[p_buy]
E[T]     ← α_size × T_bar + (1 - α_size) × E[T]
E[T]     = clamp(E[T], size_min, size_max)
threshold ← E[T] × max(E[p_buy], 1 - E[p_buy])
```

## Hyperparameters

TRBS reuses the same `TibsConfig` structure for convenience. The `alpha_imbl` parameter is repurposed as the probability EWMA alpha.

| Parameter | Config Key | Default | Description |
|-----------|-----------|---------|-------------|
| Initial bar size | `initial_size` | 100.0 | Starting estimate for `E[T]` |
| Initial buy probability | `initial_p_buy` | 0.7 | Starting estimate for `E[p_buy]` |
| Size EWMA alpha | `alpha_size` | 0.1 | Smoothing factor for bar size |
| Probability EWMA alpha | `alpha_imbl` | 0.1 | Smoothing factor for buy probability (reuses TIBS config key) |
| Size bounds | `size_min_pct` / `size_max_pct` | ±10% of initial | Clamp range for `E[T]` |

## Output

Each closed bar emits a `Candle` protobuf message:

| Field | Content |
|-------|---------|
| `symbol` | Trading pair |
| `timestamp` | Bar open time (ms) |
| `open`, `high`, `low`, `close` | OHLC prices |
| `volume` | Total traded quantity |
| `interval` | Label (e.g., `trb_small`, `trb_large`) |
| `buy_ticks` | Buyer-initiated trade count |
| `sell_ticks` | Seller-initiated trade count |
| `total_ticks` | Total trades |
| `theta` | Longest run length (unsigned) |

## Service Variants

- **trbs_small** (`port_trbs_small`): `initial_size = 100`
- **trbs_large** (`port_trbs_large`): `initial_size = 500`

## Persistence

TRBS bars are persisted to TimescaleDB in the `warehouse.bar__tick_imbalance` table (shared with TIBS). The `interval` column (e.g., `trb_small` vs `tib_small`) distinguishes bar types.

## Difference from TIBS

| Aspect | TIBS | TRBS |
|--------|------|------|
| What it measures | Net imbalance (buys - sells) | Longest consecutive run |
| Closing condition | \|Σ b_t\| > threshold | max_run > threshold |
| Sensitivity | Aggregate directional pressure | Sequential order flow |
| Use case | Overall directional bias | Algorithmic/momentum detection |

## References

- López de Prado, M. (2018). *Advances in Financial Machine Learning*. Wiley. Chapter 2: Financial Data Structures.

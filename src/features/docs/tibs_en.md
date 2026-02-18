# Tick Imbalance Bars (TIBS)

## Overview

Tick Imbalance Bars (TIBS) are an information-driven bar sampling method introduced by Marcos López de Prado in *Advances in Financial Machine Learning* (2018). Unlike time bars (which sample at fixed intervals) or tick bars (which sample every N ticks), TIBS close a bar when the **cumulative tick imbalance** exceeds a dynamically adjusted threshold. This means bars form faster during periods of high directional activity and slower during balanced markets.

## Why Use TIBS

- **Information-driven sampling**: Bars are triggered by meaningful market events (directional pressure), not arbitrary time or tick counts.
- **Better statistical properties**: Returns sampled via TIBS tend to be closer to IID (independent and identically distributed) compared to time bars.
- **Adaptive**: The threshold adjusts over time via EWMA, automatically adapting to changing market microstructure.
- **Detects imbalance**: Useful for detecting informed trading, momentum shifts, and directional conviction.

## Mathematical Foundation

### Tick Rule

Each trade is classified as a buy or sell using the tick rule:

```
b_t = +1  if trade is buyer-initiated
b_t = -1  if trade is seller-initiated
```

In Raven, the side is provided directly by the exchange (aggressor side from Binance).

### Cumulative Imbalance (Theta)

For each bar, the cumulative tick imbalance is:

```
θ_T = Σ(t=1..T) b_t
```

where T is the number of ticks accumulated so far in the current bar.

### Bar Closing Condition

A bar closes when:

```
|θ_T| > |E[T] × (2·E[p_buy] - 1)|
```

where:
- `E[T]` = expected number of ticks per bar (EWMA of historical bar sizes)
- `E[p_buy]` = expected proportion of buy ticks (EWMA of historical buy ratios)

The threshold adapts: if recent bars had more ticks, the threshold grows (bars get larger); if recent imbalance was larger, the threshold shrinks (bars form faster).

### EWMA Updates (After Each Bar Close)

When a bar closes with `T_bar` ticks and observed `p_buy`:

```
E[T]     ← α_size × T_bar + (1 - α_size) × E[T]
E[p_buy] ← α_imbl × p_buy + (1 - α_imbl) × E[p_buy]
threshold ← E[T] × (2·E[p_buy] - 1)
```

`E[T]` is clamped to `[size_min, size_max]` to prevent degenerate bar sizes.

## Hyperparameters

| Parameter | Config Key | Default | Description |
|-----------|-----------|---------|-------------|
| Initial bar size | `initial_size` | 100.0 | Starting estimate for expected ticks per bar `E[T]` |
| Initial buy probability | `initial_p_buy` | 0.7 | Starting estimate for `E[p_buy]` |
| Size EWMA alpha | `alpha_size` | 0.1 | Smoothing factor for bar size EWMA (higher = more reactive) |
| Imbalance EWMA alpha | `alpha_imbl` | 0.1 | Smoothing factor for buy probability EWMA |
| Size lower bound | `size_min` or `size_min_pct` | 10% below initial | Minimum allowed `E[T]` (prevents bars from becoming too small) |
| Size upper bound | `size_max` or `size_max_pct` | 10% above initial | Maximum allowed `E[T]` (prevents bars from becoming too large) |

### Size Bounds

Bounds can be specified two ways:
- **Percentage** (`size_min_pct`, `size_max_pct`): Relative to `initial_size`. E.g., `size_min_pct = 0.1` means `min = initial_size × 0.9`.
- **Absolute** (`size_min`, `size_max`): Fixed tick counts.

Percentage bounds take precedence if both are specified.

## Output

Each closed bar emits a `Candle` protobuf message with:

| Field | Content |
|-------|---------|
| `symbol` | Trading pair (e.g., BTCUSDT) |
| `timestamp` | Bar open time (ms since epoch) |
| `open`, `high`, `low`, `close` | OHLC prices |
| `volume` | Total traded quantity in the bar |
| `interval` | Label string (e.g., `tib_small`, `tib_large`) |
| `buy_ticks` | Number of buyer-initiated trades |
| `sell_ticks` | Number of seller-initiated trades |
| `total_ticks` | Total trade count in the bar |
| `theta` | Final cumulative tick imbalance (signed integer) |

## Service Variants

Raven runs TIBS with two profiles by default:

- **tibs_small** (`port_tibs_small`): `initial_size = 100` — produces bars frequently, sensitive to small imbalances
- **tibs_large** (`port_tibs_large`): `initial_size = 500` — produces bars less frequently, captures larger directional moves

## Persistence

TIBS bars are persisted to TimescaleDB in the `mart.bar__tick_imbalance` table. The `interval` column distinguishes between small/large profiles and TIBS vs TRBS.

## References

- López de Prado, M. (2018). *Advances in Financial Machine Learning*. Wiley. Chapter 2: Financial Data Structures.

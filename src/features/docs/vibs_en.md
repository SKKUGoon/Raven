# Volume Imbalance Bars (VIBS)

## Overview

Volume Imbalance Bars (VIBS) extend the tick imbalance concept by weighting each tick by its **trade volume** rather than counting ticks uniformly. This captures the economic significance of each trade: a 100 BTC buy contributes 100× more imbalance signal than a 1 BTC buy. Bars close when the cumulative *signed volume* exceeds a dynamically adjusted threshold.

## Why Use VIBS

- **Volume-weighted signal**: Large trades carry proportionally more weight in triggering bar closes, reflecting their greater market impact.
- **Better for institutional flow detection**: Institutional orders often trade in larger sizes. VIBS naturally amplify these signals.
- **Noise reduction**: Small retail trades contribute minimally to the imbalance, reducing noise compared to TIBS where every tick counts equally.
- **Adaptive**: Threshold adjusts via triple EWMA (bar size, buy probability, average volume per tick).

## Mathematical Foundation

### Signed Volume

Each trade contributes signed volume:

```
v_t × b_t
```

where:
- `v_t` = trade quantity (volume)
- `b_t` = +1.0 (buy) or -1.0 (sell)

### Cumulative Volume Imbalance (Theta)

```
θ_T = Σ(t=1..T) v_t × b_t
```

This is the net directional volume: positive means more buy volume, negative means more sell volume.

### Bar Closing Condition

A bar closes when:

```
|θ_T| > |E[T] × E[v] × (2·E[p_buy] - 1)|
```

where:
- `E[T]` = expected ticks per bar (EWMA)
- `E[v]` = expected volume per tick (EWMA)
- `E[p_buy]` = expected buy probability (EWMA)
- `(2·E[p_buy] - 1)` = expected direction of each tick

The threshold reflects the expected *net volume* per bar under the current market regime.

### EWMA Updates (After Each Bar Close)

When a bar closes:

```
E[p_buy] ← α_imbl × p_buy_observed + (1 - α_imbl) × E[p_buy]
E[v]     ← α_imbl × avg_vol_per_tick + (1 - α_imbl) × E[v]
E[T]     ← α_size × T_bar + (1 - α_size) × E[T]
E[T]     = clamp(E[T], size_min, size_max)
threshold ← E[T] × E[v] × (2·E[p_buy] - 1)
```

Note: `E[v]` is seeded from the first observed trade volume, since there is no prior estimate for typical trade size.

## Hyperparameters

VIBS reuses the `TibsConfig` structure.

| Parameter | Config Key | Default | Description |
|-----------|-----------|---------|-------------|
| Initial bar size | `initial_size` | 100.0 | Starting `E[T]` (ticks per bar) |
| Initial buy probability | `initial_p_buy` | 0.7 | Starting `E[p_buy]` |
| Size EWMA alpha | `alpha_size` | 0.1 | Smoothing for `E[T]` |
| Volume/probability EWMA alpha | `alpha_imbl` | 0.1 | Smoothing for both `E[v]` and `E[p_buy]` |
| Size bounds | `size_min_pct` / `size_max_pct` | ±10% of initial | Clamp range for `E[T]` |

### Key Difference from TIBS

VIBS has an additional state variable `E[v]` (expected volume per tick) that makes the threshold volume-aware. This is initialized to 0 and seeded on the first tick.

## Output

Each closed bar emits a `Candle` protobuf message:

| Field | Content |
|-------|---------|
| `symbol` | Trading pair |
| `timestamp` | Bar open time (ms) |
| `open`, `high`, `low`, `close` | OHLC prices |
| `volume` | Total traded quantity |
| `interval` | Label (e.g., `vib_small`, `vib_large`) |
| `buy_ticks` | Buyer-initiated trade count |
| `sell_ticks` | Seller-initiated trade count |
| `total_ticks` | Total trades |
| `theta` | Final cumulative signed volume imbalance (float) |

Note: Unlike TIBS where theta is an integer (tick count), VIBS theta is a float (volume-weighted).

## Service Variants

- **vibs_small** (`port_vibs_small`): `initial_size = 100`
- **vibs_large** (`port_vibs_large`): `initial_size = 250`

## Persistence

VIBS bars are persisted to TimescaleDB in the `warehouse.bar__volume_imbalance` table. The `interval` column distinguishes between small/large profiles.

## Comparison with TIBS

| Aspect | TIBS | VIBS |
|--------|------|------|
| Imbalance unit | Tick count | Volume (quote/base qty) |
| Large trade sensitivity | Every tick equal | Proportional to size |
| Threshold components | E[T] × E[imbalance] | E[T] × E[v] × E[imbalance] |
| Best for | Detecting tick-level directional bias | Detecting volume-weighted flow |
| Theta semantics | Net tick count (integer) | Net signed volume (float) |

## References

- López de Prado, M. (2018). *Advances in Financial Machine Learning*. Wiley. Chapter 2: Financial Data Structures.

# VPIN (Volume-Synchronized Probability of Informed Trading)

## Overview

VPIN is a real-time estimator of **order flow toxicity** — the probability that market makers are trading against informed counterparties. Developed by Easley, López de Prado, and O'Hara (2012), it was notably used to detect elevated toxicity hours before the May 6, 2010 Flash Crash.

Unlike TIBS/TRBS/VIBS which produce OHLCV bars, VPIN produces a continuous metric (0 to 1) that measures how "toxic" the current order flow is. High VPIN = high probability of informed trading = dangerous for market makers.

## Why Use VPIN

- **Early warning system**: Spikes in VPIN often precede large price moves and volatility events.
- **Market regime detection**: Distinguishes informed vs uninformed flow regimes.
- **Risk management**: Can trigger protective actions (wider spreads, position reduction) when toxicity is elevated.
- **Volume-time sampling**: Uses equal-volume buckets, which normalize for varying activity levels.

## Mathematical Foundation

### Step 1: Equal-Volume Buckets

Trades are accumulated into buckets of fixed notional volume `V` (in quote currency, e.g., USD). A new bucket starts when the previous bucket's cumulative notional (`Σ price × quantity`) reaches `V`.

```
Bucket closes when: Σ(price_t × qty_t) ≥ V
```

### Step 2: Bulk Volume Classification (BVC)

Instead of classifying individual trades as buy/sell, BVC uses the price change across a bucket to probabilistically assign volume. For a closed bucket with price change `ΔP`:

```
ΔP = close_price - previous_bucket_close
σ  = stddev(ΔP) over last `sigma_window` buckets
z  = ΔP / σ

V_buy  = V × Φ(z)
V_sell = V × (1 - Φ(z)) = V - V_buy
```

where `Φ(z)` is the standard normal CDF.

Intuition: If the price went up (positive ΔP), more volume is classified as buy-initiated. The magnitude relative to recent volatility (σ) determines how extreme the classification is.

### Step 3: Order Imbalance

For each bucket, the absolute order imbalance is:

```
OI = |V_buy - V_sell| = |2·V_buy - V|
```

### Step 4: Rolling VPIN

VPIN is the average order imbalance over the last `n` buckets, normalized by `V`:

```
VPIN = (Σ OI_i for i in last n buckets) / (n × V)
```

VPIN ranges from 0 (perfectly balanced) to 1 (completely one-sided).

## Hyperparameters

| Parameter | CLI Flag | Default | Description |
|-----------|----------|---------|-------------|
| Bucket volume (V) | `--v` | 10,000 | Notional volume per bucket in quote currency (USD/USDT). Larger V = smoother but slower. |
| Rolling window (n) | `--n` | 50 | Number of buckets in the VPIN rolling average. Larger n = smoother VPIN. |
| Sigma window | `--sigma-window` | 20 | Number of past price changes for σ estimation. Must be ≥ 2. |
| Sigma floor | `--sigma-floor` | 1e-12 | Minimum σ to prevent division by zero in low-volatility periods. |

### Hyperparameter Tuning Guidance

- **V (bucket volume)**: Should be calibrated to the instrument's typical notional volume. For BTC/USDT, `V = 10,000` means one bucket per ~$10,000 traded. For less liquid instruments, decrease V; for more liquid, increase it. A good rule of thumb: you want several hundred buckets per day.
- **n (rolling window)**: Controls the smoothness vs responsiveness tradeoff. `n = 50` with ~200 buckets/day gives a rolling window of ~6 hours. Decrease for faster signals (more noise), increase for smoother (more lag).
- **sigma_window**: Should be large enough for stable σ estimates (≥20) but small enough to adapt to regime changes.

## Output

VPIN output is encoded as a `Candle` protobuf message for compatibility with the existing persistence pipeline:

| Field | Content |
|-------|---------|
| `symbol` | Trading pair |
| `timestamp` | Bucket open time (ms) |
| `open`, `high`, `low`, `close` | OHLC prices within the bucket |
| `volume` | V (the configured bucket volume, constant) |
| `interval` | Config encoding (e.g., `vpin_V10000_n50_sw20`) |
| `buy_ticks` | 0 (not meaningful for VPIN) |
| `sell_ticks` | 0 (not meaningful for VPIN) |
| `total_ticks` | Number of trade prints in the bucket |
| `theta` | **The VPIN value** (0.0 to 1.0) |

The VPIN value is carried in the `theta` field to avoid adding a new proto field.

## Implementation Details

### Warmup Period

VPIN requires a warmup period before it can emit values:
1. The first bucket needs a `prev_close` to compute ΔP → so the first bucket is silent.
2. `sigma_window` buckets are needed before σ is computable.
3. `n` buckets are needed before the rolling VPIN average is full.

Total warmup: ~(`sigma_window` + `n`) buckets.

### CDF Approximation

The standard normal CDF `Φ(x)` is computed using the Abramowitz & Stegun (1964) approximation of `erf(x)`, which is fast and sufficiently accurate (max error ~1.5×10⁻⁷).

### Seeding

The first observed trade price is used to seed `prev_close`. Until seeded, no buckets can produce valid ΔP values.

## Interpretation

| VPIN Range | Interpretation |
|-----------|----------------|
| 0.0 – 0.3 | Low toxicity — balanced flow, safe for market making |
| 0.3 – 0.5 | Moderate — some informed activity |
| 0.5 – 0.7 | Elevated — increased informed trading probability |
| 0.7 – 1.0 | High toxicity — strong informed flow, risk of adverse selection |

These ranges are approximate and should be calibrated per instrument and market.

## Persistence

VPIN buckets are persisted to TimescaleDB in the `warehouse.bar__vpin` table. The `interval` column encodes the config parameters for identification.

## References

- Easley, D., López de Prado, M.M., & O'Hara, M. (2012). Flow Toxicity and Liquidity in a High-frequency World. *Review of Financial Studies*, 25(5), 1457–1493.
- Easley, D., López de Prado, M.M., & O'Hara, M. (2011). The Microstructure of the "Flash Crash": Flow Toxicity, Liquidity Crashes, and the Probability of Informed Trading. *Journal of Portfolio Management*, 37(2), 118–128.
- López de Prado, M. (2018). *Advances in Financial Machine Learning*. Wiley. Chapter 19: Microstructural Features.

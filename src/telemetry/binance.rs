use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, register_int_gauge, IntCounterVec, IntGauge};

lazy_static! {
    // Binance Futures Source Metrics
    pub static ref BINANCE_FUTURES_TRADES: IntCounterVec = register_int_counter_vec!(
        "raven_binance_futures_trades_processed_total",
        "Total number of trades processed",
        &["symbol"]
    )
    .unwrap();
    pub static ref BINANCE_FUTURES_CONNECTIONS: IntGauge = register_int_gauge!(
        "raven_binance_futures_active_connections",
        "Number of active WebSocket connections"
    )
    .unwrap();

    // Binance Futures Klines Source Metrics
    pub static ref BINANCE_FUTURES_KLINES_PROCESSED: IntCounterVec = register_int_counter_vec!(
        "raven_binance_futures_klines_processed_total",
        "Total number of closed klines processed",
        &["symbol"]
    )
    .unwrap();
    pub static ref BINANCE_FUTURES_KLINES_CONNECTIONS: IntGauge = register_int_gauge!(
        "raven_binance_futures_klines_active_connections",
        "Number of active WebSocket connections (kline shards)"
    )
    .unwrap();

    // Binance Spot Source Metrics
    pub static ref BINANCE_SPOT_TRADES: IntCounterVec = register_int_counter_vec!(
        "raven_binance_spot_trades_processed_total",
        "Total number of trades processed",
        &["symbol"]
    )
    .unwrap();
    pub static ref BINANCE_SPOT_CONNECTIONS: IntGauge = register_int_gauge!(
        "raven_binance_spot_active_connections",
        "Number of active WebSocket connections"
    )
    .unwrap();
}

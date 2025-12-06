use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, register_int_gauge, IntCounterVec, IntGauge};

lazy_static! {
    // InfluxDB Persistence Metrics
    pub static ref INFLUX_POINTS_WRITTEN: IntCounterVec = register_int_counter_vec!(
        "raven_persistence_points_written_total",
        "Total number of data points written to InfluxDB",
        &["bucket"]
    )
    .unwrap();
    pub static ref INFLUX_ACTIVE_TASKS: IntGauge = register_int_gauge!(
        "raven_persistence_active_tasks",
        "Number of active persistence tasks"
    )
    .unwrap();

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

    // Tibs Feature Metrics
    pub static ref TIBS_GENERATED: IntCounterVec = register_int_counter_vec!(
        "raven_tibs_generated_total",
        "Total number of TIBs generated",
        &["symbol"]
    )
    .unwrap();
    pub static ref TIBS_ACTIVE_AGGREGATIONS: IntGauge = register_int_gauge!(
        "raven_tibs_active_aggregations",
        "Number of active TIB aggregation tasks"
    )
    .unwrap();

    // TimeBar Minutes Feature Metrics
    pub static ref TIMEBAR_MINUTES_GENERATED: IntCounterVec = register_int_counter_vec!(
        "raven_timebar_minutes_generated_total",
        "Total number of timebar minutes generated",
        &["symbol"]
    )
    .unwrap();
    pub static ref TIMEBAR_MINUTES_ACTIVE_AGGREGATIONS: IntGauge = register_int_gauge!(
        "raven_timebar_minutes_active_aggregations",
        "Number of active timebar minutes aggregation tasks"
    )
    .unwrap();
}


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
}

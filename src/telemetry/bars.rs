use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, register_int_gauge, IntCounterVec, IntGauge};

lazy_static! {
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

    // Vibs (Volume Imbalance Bars) Feature Metrics
    pub static ref VIBS_GENERATED: IntCounterVec = register_int_counter_vec!(
        "raven_vibs_generated_total",
        "Total number of VIBs generated",
        &["symbol"]
    )
    .unwrap();
    pub static ref VIBS_ACTIVE_AGGREGATIONS: IntGauge = register_int_gauge!(
        "raven_vibs_active_aggregations",
        "Number of active VIB aggregation tasks"
    )
    .unwrap();

    // Trbs (Tick Run Bars) Feature Metrics
    pub static ref TRBS_GENERATED: IntCounterVec = register_int_counter_vec!(
        "raven_trbs_generated_total",
        "Total number of TRBs generated",
        &["symbol"]
    )
    .unwrap();
    pub static ref TRBS_ACTIVE_AGGREGATIONS: IntGauge = register_int_gauge!(
        "raven_trbs_active_aggregations",
        "Number of active TRB aggregation tasks"
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

    // VPIN Feature Metrics
    pub static ref VPIN_GENERATED: IntCounterVec = register_int_counter_vec!(
        "raven_vpin_generated_total",
        "Total number of VPIN buckets emitted",
        &["symbol"]
    )
    .unwrap();
    pub static ref VPIN_ACTIVE_AGGREGATIONS: IntGauge = register_int_gauge!(
        "raven_vpin_active_aggregations",
        "Number of active VPIN aggregation tasks"
    )
    .unwrap();
}

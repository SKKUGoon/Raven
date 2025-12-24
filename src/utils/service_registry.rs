use crate::config::Settings;

#[derive(Clone, Debug)]
pub struct ServiceSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub bin_name: &'static str,
    pub log_name: &'static str,
    pub port: u16,
    pub args: Vec<String>,
}

impl ServiceSpec {
    pub fn addr(&self, host: &str) -> String {
        format!("http://{}:{}", host, self.port)
    }
}

pub fn all_services(settings: &Settings) -> Vec<ServiceSpec> {
    vec![
        ServiceSpec {
            id: "binance_spot",
            display_name: "Binance Spot",
            bin_name: "binance_spot",
            log_name: "binance_spot",
            port: settings.server.port_binance_spot,
            args: vec![],
        },
        ServiceSpec {
            id: "binance_futures",
            display_name: "Binance Futures",
            bin_name: "binance_futures",
            log_name: "binance_futures",
            port: settings.server.port_binance_futures,
            args: vec![],
        },
        ServiceSpec {
            id: "timebar_60s",
            display_name: "TimeBar (1m)",
            bin_name: "raven_timebar",
            log_name: "timebar_1m",
            port: settings.server.port_timebar_minutes,
            args: vec![
                "--seconds".to_string(),
                "60".to_string(),
                "--port".to_string(),
                settings.server.port_timebar_minutes.to_string(),
            ],
        },
        ServiceSpec {
            id: "timebar_1s",
            display_name: "TimeBar (1s)",
            bin_name: "raven_timebar",
            log_name: "timebar_1s",
            port: settings.server.port_timebar_seconds,
            args: vec![
                "--seconds".to_string(),
                "1".to_string(),
                "--port".to_string(),
                settings.server.port_timebar_seconds.to_string(),
            ],
        },
        ServiceSpec {
            id: "tibs",
            display_name: "Tibs",
            bin_name: "raven_tibs",
            log_name: "tibs",
            port: settings.server.port_tibs,
            args: vec![],
        },
        ServiceSpec {
            id: "tick_persistence",
            display_name: "Tick Persistence",
            bin_name: "tick_persistence",
            log_name: "tick_persistence",
            port: settings.server.port_tick_persistence,
            args: vec![],
        },
        ServiceSpec {
            id: "bar_persistence",
            display_name: "Bar Persistence",
            bin_name: "bar_persistence",
            log_name: "bar_persistence",
            port: settings.server.port_bar_persistence,
            args: vec![],
        },
    ]
}



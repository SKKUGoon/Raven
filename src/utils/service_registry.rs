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

/// If services bind to `0.0.0.0` (listen on all interfaces), clients should connect to a
/// routable address like `127.0.0.1` instead of `0.0.0.0`.
pub fn client_host(host: &str) -> &str {
    match host {
        "0.0.0.0" | "::" => "127.0.0.1",
        _ => host,
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
            id: "tibs_small",
            display_name: "Tibs (small)",
            bin_name: "raven_tibs_small",
            log_name: "tibs_small",
            port: settings.server.port_tibs_small,
            args: vec![],
        },
        ServiceSpec {
            id: "tibs_large",
            display_name: "Tibs (large)",
            bin_name: "raven_tibs_large",
            log_name: "tibs_large",
            port: settings.server.port_tibs_large,
            args: vec![],
        },
        ServiceSpec {
            id: "trbs_small",
            display_name: "Trbs (small)",
            bin_name: "raven_trbs_small",
            log_name: "trbs_small",
            port: settings.server.port_trbs_small,
            args: vec![],
        },
        ServiceSpec {
            id: "trbs_large",
            display_name: "Trbs (large)",
            bin_name: "raven_trbs_large",
            log_name: "trbs_large",
            port: settings.server.port_trbs_large,
            args: vec![],
        },
        ServiceSpec {
            id: "vibs_small",
            display_name: "Vibs (small)",
            bin_name: "raven_vibs_small",
            log_name: "vibs_small",
            port: settings.server.port_vibs_small,
            args: vec![],
        },
        ServiceSpec {
            id: "vibs_large",
            display_name: "Vibs (large)",
            bin_name: "raven_vibs_large",
            log_name: "vibs_large",
            port: settings.server.port_vibs_large,
            args: vec![],
        },
        ServiceSpec {
            id: "vpin",
            display_name: "VPIN",
            bin_name: "raven_vpin",
            log_name: "vpin",
            port: settings.server.port_vpin,
            args: vec![
                // Explicitly bind to the configured port (so multiple instances are possible).
                "--port".to_string(),
                settings.server.port_vpin.to_string(),
            ],
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



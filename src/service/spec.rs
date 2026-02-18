#[derive(Clone, Debug)]
pub struct ImbalanceBarSpec {
    pub initial_size: f64,
    pub initial_p_buy: f64,
    pub alpha_size: f64,
    pub alpha_imbl: f64,
    pub size_min_pct: Option<f64>,
    pub size_max_pct: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct ServiceSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub bin_name: &'static str,
    pub log_name: &'static str,
    pub port: u16,
    /// If explicitly set, these args are passed verbatim to the process.
    pub args: Vec<String>,
    /// Optional candle interval label for bar aggregators (e.g. `tib_small`, `trb_large`).
    pub interval: Option<String>,
    /// Optional bar config overrides for imbalance/run/volume imbalance services.
    pub imbalance: Option<ImbalanceBarSpec>,
}

impl ServiceSpec {
    pub fn addr(&self, host: &str) -> String {
        format!("http://{}:{}", host, self.port)
    }

    /// Effective command-line args for launching this service.
    ///
    /// - If `args` is explicitly set, we use it verbatim.
    /// - Otherwise, if `interval`/`imbalance` is present, we synthesize args for the unified
    ///   `raven_{tibs,trbs,vibs}` binaries.
    pub fn effective_args(&self) -> Vec<String> {
        if !self.args.is_empty() {
            return self.args.clone();
        }

        let mut out = Vec::new();
        if let Some(interval) = &self.interval {
            out.push("--interval".to_string());
            out.push(interval.clone());
        }

        if self.interval.is_some() || self.imbalance.is_some() {
            out.push("--port".to_string());
            out.push(self.port.to_string());
        }

        if let Some(cfg) = &self.imbalance {
            out.push("--initial-size".to_string());
            out.push(cfg.initial_size.to_string());
            out.push("--initial-p-buy".to_string());
            out.push(cfg.initial_p_buy.to_string());
            out.push("--alpha-size".to_string());
            out.push(cfg.alpha_size.to_string());
            out.push("--alpha-imbl".to_string());
            out.push(cfg.alpha_imbl.to_string());

            if let Some(v) = cfg.size_min_pct {
                out.push("--size-min-pct".to_string());
                out.push(v.to_string());
            }
            if let Some(v) = cfg.size_max_pct {
                out.push("--size-max-pct".to_string());
                out.push(v.to_string());
            }
        }

        out
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



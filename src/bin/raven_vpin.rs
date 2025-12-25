use clap::Parser;
use raven::config::Settings;
use raven::features::vpin;
use raven::features::vpin::VpinConfig;
use raven::service::RavenService;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "raven_vpin")]
#[command(about = "VPIN (Volume-synchronized Probability of Informed Trading) aggregator")]
struct Cli {
    /// Volume per bucket (V)
    #[arg(long, default_value_t = 10_000.0)]
    v: f64,

    /// Rolling number of buckets for VPIN (n)
    #[arg(long, default_value_t = 50)]
    n: usize,

    /// Rolling window in buckets for sigma estimation (sigma_window)
    #[arg(long, default_value_t = 20)]
    sigma_window: usize,

    /// Numerical floor for sigma
    #[arg(long, default_value_t = 1e-12)]
    sigma_floor: f64,

    /// Override listening port
    #[arg(short, long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let settings = Settings::new()?;

    let log_level = match settings.logging.level.to_lowercase().as_str() {
        "debug" => tracing::Level::DEBUG,
        "error" => tracing::Level::ERROR,
        "warn" => tracing::Level::WARN,
        _ => tracing::Level::INFO,
    };
    tracing_subscriber::fmt().with_max_level(log_level).init();

    let port = cli.port.unwrap_or(settings.server.port_vpin);
    let addr = format!("{}:{}", settings.server.host, port).parse()?;

    let mut upstreams = HashMap::new();
    upstreams.insert(
        "BINANCE_SPOT".to_string(),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_binance_spot
        ),
    );
    upstreams.insert(
        "BINANCE_FUTURES".to_string(),
        format!(
            "http://{}:{}",
            settings.server.host, settings.server.port_binance_futures
        ),
    );

    let cfg = VpinConfig {
        v: cli.v,
        n: cli.n,
        sigma_window: cli.sigma_window,
        sigma_floor: cli.sigma_floor,
    };

    // Encode config in the interval string so downstream persistence can distinguish variants.
    let interval = format!("vpin_V{}_n{}_sw{}", cfg.v, cfg.n, cfg.sigma_window);

    let service_impl = vpin::new(upstreams, cfg, interval);
    let raven = RavenService::new("RavenVpin", service_impl.clone());
    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}



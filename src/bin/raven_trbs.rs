use clap::Parser;
use raven::config::{Settings, TibsConfig};
use raven::features::trbs;
use raven::service::RavenService;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "raven_trbs")]
#[command(about = "Tick run bars (TRBS) aggregator", long_about = None)]
struct Cli {
    /// Candle interval label (e.g. trb_small)
    #[arg(long, default_value = "trb")]
    interval: String,

    /// Override listening port
    #[arg(short, long)]
    port: Option<u16>,

    #[arg(long)]
    initial_size: Option<f64>,
    #[arg(long)]
    initial_p_buy: Option<f64>,
    #[arg(long)]
    alpha_size: Option<f64>,
    #[arg(long)]
    alpha_imbl: Option<f64>,
    #[arg(long)]
    size_min: Option<f64>,
    #[arg(long)]
    size_max: Option<f64>,
    #[arg(long)]
    size_min_pct: Option<f64>,
    #[arg(long)]
    size_max_pct: Option<f64>,
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

    let port = cli.port.unwrap_or(settings.server.port_trbs_small);
    let addr = format!("{}:{}", settings.server.host, port).parse()?;

    let interval = cli.interval;

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

    let base = settings.tibs.clone();
    let config = TibsConfig {
        initial_size: cli.initial_size.unwrap_or(base.initial_size),
        initial_p_buy: cli.initial_p_buy.unwrap_or(base.initial_p_buy),
        alpha_size: cli.alpha_size.unwrap_or(base.alpha_size),
        alpha_imbl: cli.alpha_imbl.unwrap_or(base.alpha_imbl),
        size_min: cli.size_min.or(base.size_min),
        size_max: cli.size_max.or(base.size_max),
        size_min_pct: cli.size_min_pct.or(base.size_min_pct),
        size_max_pct: cli.size_max_pct.or(base.size_max_pct),
        profiles: Default::default(),
    };

    let service_impl = trbs::new(upstreams, config, interval.clone());
    let service_name = format!("RavenTrbs_{interval}");
    let raven = RavenService::new(&service_name, service_impl.clone());
    raven.serve_with_market_data(addr, service_impl).await?;
    Ok(())
}



use clap::Parser;
use raven::config::Settings;
use raven::features::tibs;
use raven::service::RavenService;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "raven_tibs")]
#[command(about = "Tick Imbalance Bar aggregator", long_about = None)]
struct Cli {
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut settings = Settings::new()?;

    // Override settings with CLI arguments if provided
    if let Some(v) = cli.initial_size {
        settings.tibs.initial_size = v;
    }
    if let Some(v) = cli.initial_p_buy {
        settings.tibs.initial_p_buy = v;
    }
    if let Some(v) = cli.alpha_size {
        settings.tibs.alpha_size = v;
    }
    if let Some(v) = cli.alpha_imbl {
        settings.tibs.alpha_imbl = v;
    }
    if let Some(v) = cli.size_min {
        settings.tibs.size_min = v;
    }
    if let Some(v) = cli.size_max {
        settings.tibs.size_max = v;
    }

    let log_level = match settings.logging.level.to_lowercase().as_str() {
        "debug" => tracing::Level::DEBUG,
        "error" => tracing::Level::ERROR,
        "warn" => tracing::Level::WARN,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt().with_max_level(log_level).init();

    let addr = format!("{}:{}", settings.server.host, settings.server.port_tibs).parse()?;

    let mut upstreams = HashMap::new();
    // Standard names
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

    let service_impl = tibs::new(upstreams, settings.tibs.clone());
    let raven = RavenService::new("RavenTibs", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}

use clap::Parser;
use raven::config::Settings;
use raven::features::timebar;
use raven::service::RavenService;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "raven_timebar")]
#[command(about = "Time-based bar aggregator", long_about = None)]
struct Cli {
    /// Bar interval in seconds
    #[arg(short = 'k', long, default_value_t = 60)]
    seconds: u64,
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

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_timebar_minutes
    )
    .parse()?;

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

    let service_impl = timebar::new(upstreams, cli.seconds);
    let raven = RavenService::new("RavenTimeBar", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}

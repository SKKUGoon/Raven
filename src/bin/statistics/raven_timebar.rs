#[path = "../common/mod.rs"]
mod common;

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

    /// Override listening port
    #[arg(short, long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let settings = Settings::new()?;

    common::init_logging(&settings);

    let port = cli.port.unwrap_or(settings.server.port_timebar_minutes);
    let addr = format!("{}:{}", settings.server.host, port).parse()?;

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
    let service_name = format!("RavenTimeBar_{}s", cli.seconds);
    let raven = RavenService::new(&service_name, service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;

    Ok(())
}

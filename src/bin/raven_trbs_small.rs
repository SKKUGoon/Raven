use raven::config::{Settings, TibsConfig};
use raven::features::trbs;
use raven::service::RavenService;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    let log_level = match settings.logging.level.to_lowercase().as_str() {
        "debug" => tracing::Level::DEBUG,
        "error" => tracing::Level::ERROR,
        "warn" => tracing::Level::WARN,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt().with_max_level(log_level).init();

    // NOTE: We intentionally do NOT read trbs settings from TOML for this binary.
    // This variant is hardcoded.
    //
    // Config is reused from `TibsConfig`:
    // - initial_size: initial expected bar size E[T]
    // - initial_p_buy: initial probability of buy ticks p
    // - alpha_size: EWMA alpha for E[T]
    // - alpha_imbl: EWMA alpha for p (probability buy)
    let config = TibsConfig {
        initial_size: 100.0,
        initial_p_buy: 0.7,
        alpha_size: 0.7,
        alpha_imbl: 0.72,
        size_min: Some(110.0),
        size_max: Some(90.0),
        size_min_pct: None,
        size_max_pct: None,
        profiles: Default::default(),
    };

    let addr = format!(
        "{}:{}",
        settings.server.host, settings.server.port_trbs_small
    )
    .parse()?;

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

    let service_impl = trbs::new(upstreams, config, "trb_small".to_string());
    let raven = RavenService::new("RavenTrbsSmall", service_impl.clone());

    raven.serve_with_market_data(addr, service_impl).await?;
    Ok(())
}



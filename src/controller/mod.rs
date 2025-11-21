// Raven Control CLI - Command line interface for managing Raven data collection
// "The Hand of the King's command interface"

use clap::{Arg, Command};
use crate::common::config::{ConfigLoader, ConfigUtils, RuntimeConfig};
use crate::common::time::current_timestamp_millis;
use crate::common::error::{RavenError, RavenResult};
use crate::proto::control_service_client::ControlServiceClient;
use crate::proto::{ListCollectionsRequest, StartCollectionRequest, StopCollectionRequest};
use std::path::PathBuf;
use tracing::{error, info};

/// Application version information
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default server address
const DEFAULT_SERVER: &str = "http://127.0.0.1:50052"; // Control service port

/// Parse command line arguments and execute the appropriate command
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for configuration commands; ignore if already set.
    let _ = tracing_subscriber::fmt::try_init();

    let matches = Command::new("ravenctl")
        .version(VERSION)
        .author("The Raven Team")
        .about("Raven Control CLI - Manage data collection and configuration")
        .long_about(format!(
            "Raven Control CLI - Command line interface for managing Raven data collection and configuration\n\
             Version: {VERSION}\n\n\
             Use this tool to start, stop, and list active data collections or validate and inspect configuration."
        ))
        .arg(
            Arg::new("server")
                .short('s')
                .long("server")
                .value_name("ADDRESS")
                .help("Raven control server address")
                .long_help("Address of the running Raven control server. Default: http://127.0.0.1:50052")
                .default_value(DEFAULT_SERVER),
        )
        .subcommand(
            Command::new("use")
                .about("Start collecting data for a symbol")
                .arg(
                    Arg::new("exchange")
                        .short('e')
                        .long("exchange")
                        .value_name("EXCHANGE")
                        .help("Exchange to collect from")
                        .long_help("Exchange to collect data from. Supported: binance_futures, binance_spot")
                        .required(true),
                )
                .arg(
                    Arg::new("symbol")
                        .short('s')
                        .long("symbol")
                        .value_name("SYMBOL")
                        .help("Trading symbol to collect")
                        .long_help("Trading symbol to collect data for (e.g., BTCUSDT)")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("stop")
                .about("Stop collecting data for a symbol")
                .arg(
                    Arg::new("exchange")
                        .short('e')
                        .long("exchange")
                        .value_name("EXCHANGE")
                        .help("Exchange to stop collecting from")
                        .long_help("Exchange to stop collecting data from. Supported: binance_futures, binance_spot")
                        .required(true),
                )
                .arg(
                    Arg::new("symbol")
                        .short('s')
                        .long("symbol")
                        .value_name("SYMBOL")
                        .help("Trading symbol to stop collecting")
                        .long_help("Trading symbol to stop collecting data for (e.g., BTCUSDT)")
                        .required(true),
                ),
        )
        .subcommand(Command::new("list").about("List all active collections"))
        .subcommand(
            Command::new("validate")
                .about("Validate current configuration")
                .arg(
                    Arg::new("config-path")
                        .short('c')
                        .long("config")
                        .value_name("PATH")
                        .help("Path to configuration file"),
                ),
        )
        .subcommand(
            Command::new("show")
                .about("Show current configuration")
                .arg(
                    Arg::new("format")
                        .short('f')
                        .long("format")
                        .value_name("FORMAT")
                        .help("Output format (pretty, json)")
                        .default_value("pretty"),
                ),
        )
        .subcommand(Command::new("health").about("Check configuration health and show warnings"))
        .subcommand(
            Command::new("template")
                .about("Generate configuration template")
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("FILE")
                        .help("Output file path"),
                ),
        )
        .get_matches();

    let server_addr = matches
        .get_one::<String>("server")
        .cloned()
        .unwrap_or_else(|| DEFAULT_SERVER.to_string());

    match matches.subcommand() {
        Some(("use", sub_matches)) => {
            let mut client = ControlServiceClient::connect(server_addr.clone()).await?;
            println!("✓ Connected to Raven control server at {server_addr}");

            let exchange = sub_matches.get_one::<String>("exchange").unwrap();
            let symbol = sub_matches.get_one::<String>("symbol").unwrap();

            println!("🟢 Starting collection for {exchange}:{symbol}");

            let request = StartCollectionRequest {
                exchange: exchange.clone(),
                symbol: symbol.clone(),
            };

            match client.start_collection(request).await {
                Ok(response) => {
                    let resp = response.into_inner();
                    if resp.success {
                        println!("{}", resp.message);
                        println!("   Collection ID: {}", resp.collection_id);
                    } else {
                        println!("{}", resp.message);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    println!("Failed to start collection: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(("stop", sub_matches)) => {
            let mut client = ControlServiceClient::connect(server_addr.clone()).await?;
            println!("✓ Connected to Raven control server at {server_addr}");

            let exchange = sub_matches.get_one::<String>("exchange").unwrap();
            let symbol = sub_matches.get_one::<String>("symbol").unwrap();

            println!("🛑 Stopping collection for {exchange}:{symbol}");

            let request = StopCollectionRequest {
                exchange: exchange.clone(),
                symbol: symbol.clone(),
            };

            match client.stop_collection(request).await {
                Ok(response) => {
                    let resp = response.into_inner();
                    if resp.success {
                        println!("{}", resp.message);
                    } else {
                        println!("{}", resp.message);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    println!("Failed to stop collection: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(("list", _)) => {
            handle_list(&server_addr).await?;
        }
        Some(("validate", sub_matches)) => {
            if let Err(e) = validate_config(sub_matches.get_one::<String>("config-path")).await {
                log_and_exit(e, "Failed to validate configuration");
            }
        }
        Some(("show", sub_matches)) => {
            let format = sub_matches.get_one::<String>("format").unwrap();
            if let Err(e) = show_config(format).await {
                log_and_exit(e, "Failed to show configuration");
            }
        }
        Some(("health", _)) => {
            if let Err(e) = check_health().await {
                log_and_exit(e, "Failed to check configuration health");
            }
        }
        Some(("template", sub_matches)) => {
            if let Err(e) = generate_template(sub_matches.get_one::<String>("output")) {
                log_and_exit(e, "Failed to generate configuration template");
            }
        }
        _ => {
            println!("❌ No command specified. Use --help for usage information.");
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn handle_list(server_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ControlServiceClient::connect(server_addr.to_string()).await?;
    println!("✓ Connected to Raven control server at {server_addr}");
    println!("📋 Active collections:");

    let request = ListCollectionsRequest {};
    match client.list_collections(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            if resp.collections.is_empty() {
                println!("   No active collections");
            } else {
                println!(
                    "   {:<20} {:<12} {:<20} {:<10} Uptime",
                    "Exchange", "Symbol", "Collection ID", "Status"
                );
                println!("   {}", "-".repeat(80));

                for collection in resp.collections {
                    let uptime = format_uptime(collection.started_at);
                    println!(
                        "   {:<20} {:<12} {:<20} {:<10} {}",
                        collection.exchange,
                        collection.symbol,
                        &collection.collection_id[..8],
                        collection.status,
                        uptime
                    );
                }
            }
        }
        Err(e) => {
            println!("Failed to list collections: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Format uptime duration in a human-readable way
fn format_uptime(started_at: i64) -> String {
    let now = current_timestamp_millis();

    let duration_ms = now - started_at;

    if duration_ms < 1000 {
        format!("{duration_ms}ms")
    } else if duration_ms < 60_000 {
        format!("{:.1}s", duration_ms as f64 / 1000.0)
    } else if duration_ms < 3_600_000 {
        format!("{:.1}m", duration_ms as f64 / 60_000.0)
    } else {
        format!("{:.1}h", duration_ms as f64 / 3_600_000.0)
    }
}

fn loader_from_path(path: Option<&String>) -> ConfigLoader {
    let mut loader = ConfigLoader::new();
    if let Some(p) = path {
        loader = loader.with_file(PathBuf::from(p));
    }
    loader
}

async fn validate_config(config_path: Option<&String>) -> RavenResult<()> {
    info!("⚬ Validating configuration...");

    let loader = loader_from_path(config_path);
    let config = RuntimeConfig::load_with_loader(&loader)?;
    info!("✓ Configuration loaded successfully");

    if let Err(e) = config.validate() {
        error!("✗ Configuration validation failed: {}", e);
        return Err(e);
    }

    info!("✓ Configuration validation passed");
    ConfigUtils::print_config(&config);

    Ok(())
}

async fn show_config(format: &str) -> RavenResult<()> {
    let config = RuntimeConfig::load()?;

    match format {
        "json" => {
            let json = ConfigUtils::export_as_json(&config)?;
            println!("{json}");
        }
        "pretty" => {
            ConfigUtils::print_config(&config);
        }
        _ => {
            ConfigUtils::print_config(&config);
        }
    }

    Ok(())
}

async fn check_health() -> RavenResult<()> {
    info!("⚕ Checking configuration health...");

    let config = RuntimeConfig::load()?;
    let warnings = ConfigUtils::check_configuration_health(&config);

    if warnings.is_empty() {
        info!("✓ No configuration health issues found");
    } else {
        info!("⚠ Found {} configuration warnings:", warnings.len());
        for (i, warning) in warnings.iter().enumerate() {
            info!("  {}. {}", i + 1, warning);
        }
    }

    let summary = ConfigUtils::get_config_summary(&config);
    info!("Configuration Summary:");
    for (key, value) in summary {
        info!("  {}: {}", key, value);
    }

    Ok(())
}

fn generate_template(output_path: Option<&String>) -> RavenResult<()> {
    let template = ConfigUtils::generate_config_template();

    match output_path {
        Some(path) => {
            std::fs::write(path, template)?;
            info!("✓ Configuration template written to: {path}");
        }
        None => {
            println!("{template}");
        }
    }

    Ok(())
}

fn log_and_exit(error: RavenError, context: &str) -> ! {
    error!("{context}: {error}");
    eprintln!("{context}: {error}");
    std::process::exit(1);
}


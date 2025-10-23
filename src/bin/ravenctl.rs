// Raven Control CLI - Command line interface for managing Raven data collection
// "The Hand of the King's command interface"

use clap::{Arg, Command};
use raven::current_timestamp_millis;
use raven::proto::control_service_client::ControlServiceClient;
use raven::proto::{ListCollectionsRequest, StartCollectionRequest, StopCollectionRequest};

/// Application version information
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default server address
const DEFAULT_SERVER: &str = "http://127.0.0.1:50052"; // Control service port

/// Parse command line arguments and execute the appropriate command
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("ravenctl")
        .version(VERSION)
        .author("The Crow's Watch")
        .about("Raven Control CLI - Manage data collection")
        .long_about(format!(
            "Raven Control CLI - Command line interface for managing Raven data collection\n\
             Version: {VERSION}\n\n\
             Use this tool to start, stop, and list active data collections on a running Raven server."
        ))
        .arg(
            Arg::new("server")
                .short('s')
                .long("server")
                .value_name("ADDRESS")
                .help("Raven control server address")
                .long_help("Address of the running Raven control server. Default: http://127.0.0.1:50052")
                .default_value(DEFAULT_SERVER)
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
                        .required(true)
                )
                .arg(
                    Arg::new("symbol")
                        .short('s')
                        .long("symbol")
                        .value_name("SYMBOL")
                        .help("Trading symbol to collect")
                        .long_help("Trading symbol to collect data for (e.g., BTCUSDT)")
                        .required(true)
                )
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
                        .required(true)
                )
                .arg(
                    Arg::new("symbol")
                        .short('s')
                        .long("symbol")
                        .value_name("SYMBOL")
                        .help("Trading symbol to stop collecting")
                        .long_help("Trading symbol to stop collecting data for (e.g., BTCUSDT)")
                        .required(true)
                )
        )
        .subcommand(
            Command::new("list")
                .about("List all active collections")
        )
        .get_matches();

    let server_addr = matches.get_one::<String>("server").unwrap();

    // Connect to the control service
    let mut client = ControlServiceClient::connect(server_addr.clone()).await?;
    println!("✓ Connected to Raven control server at {server_addr}");

    match matches.subcommand() {
        Some(("use", sub_matches)) => {
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
        }
        _ => {
            println!("❌ No command specified. Use --help for usage information.");
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

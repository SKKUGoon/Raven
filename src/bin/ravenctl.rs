use clap::{Parser, Subcommand};
use raven::config::Settings;
use raven::proto::control_client::ControlClient;
use raven::proto::{ControlRequest, ListRequest, StopAllRequest};

#[derive(Parser)]
#[command(name = "ravenctl")]
#[command(about = "Control Raven services", long_about = None)]
struct Cli {
    #[arg(long, default_value = "http://localhost:50051")]
    host: String,

    /// Target service: binance_spot, binance_futures, tick_persistence, bar_persistence, timebar_minutes, tibs
    #[arg(short, long)]
    service: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start data collection for a symbol
    Start {
        #[arg(short, long)]
        symbol: String,
        #[arg(short, long)]
        exchange: Option<String>,
    },
    /// Stop data collection for a symbol
    Stop {
        #[arg(short, long)]
        symbol: String,
        #[arg(short, long)]
        exchange: Option<String>,
    },
    /// Stop all data collections
    StopAll,
    /// List active collections
    List,
    /// Check status of all services
    Status,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let settings = Settings::new().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {e}. Using defaults.");
        // Return a dummy settings object or handle gracefully?
        // For simplicity, let's just panic or exit if config is critical.
        // But since this is a CLI, maybe we can proceed if the user provided explicit host?
        // However, the logic below depends on settings for service shortcuts.
        std::process::exit(1);
    });

    // Handle Status command separately as it iterates all services
    if let Commands::Status = cli.command {
        check_status(&settings).await;
        return Ok(());
    }

    let host = if let Some(s) = cli.service {
        let host_ip = &settings.server.host;
        match s.as_str() {
            "binance_spot" => format!("http://{}:{}", host_ip, settings.server.port_binance_spot),
            "binance_futures" => format!(
                "http://{}:{}",
                host_ip, settings.server.port_binance_futures
            ),
            "tick_persistence" | "persistence" => {
                format!(
                    "http://{}:{}",
                    host_ip, settings.server.port_tick_persistence
                )
            }
            "bar_persistence" => {
                format!(
                    "http://{}:{}",
                    host_ip, settings.server.port_bar_persistence
                )
            }
            "timebar" | "timebar_minutes" => format!(
                "http://{}:{}",
                host_ip, settings.server.port_timebar_minutes
            ),
            "tibs" => format!("http://{}:{}", host_ip, settings.server.port_tibs),
            _ => {
                eprintln!("Unknown service: {s}. Using default host.");
                cli.host
            }
        }
    } else {
        cli.host
    };

    println!("Connecting to {host}");
    let mut client = ControlClient::connect(host).await?;

    match cli.command {
        Commands::Start { symbol, exchange } => {
            let request = ControlRequest {
                symbol,
                exchange: exchange.unwrap_or_default(),
            };
            let response = client.start_collection(request).await?.into_inner();
            println!("Success: {}", response.success);
            println!("Message: {}", response.message);
        }
        Commands::Stop { symbol, exchange } => {
            let request = ControlRequest {
                symbol,
                exchange: exchange.unwrap_or_default(),
            };
            let response = client.stop_collection(request).await?.into_inner();
            println!("Success: {}", response.success);
            println!("Message: {}", response.message);
        }
        Commands::StopAll => {
            let response = client
                .stop_all_collections(StopAllRequest {})
                .await?
                .into_inner();
            println!("Success: {}", response.success);
            println!("Message: {}", response.message);
        }
        Commands::List => {
            let response = client.list_collections(ListRequest {}).await?.into_inner();
            println!("Active Collections:");
            for collection in response.collections {
                println!(
                    "- Symbol: {}, Status: {}, Subscribers: {}",
                    collection.symbol, collection.status, collection.subscriber_count
                );
            }
        }
        Commands::Status => unreachable!(), // Handled above
    }

    Ok(())
}

async fn check_status(settings: &Settings) {
    let services = vec![
        ("Binance Spot", settings.server.port_binance_spot),
        ("Binance Futures", settings.server.port_binance_futures),
        ("TimeBar", settings.server.port_timebar_minutes),
        ("Tibs", settings.server.port_tibs),
        ("Tick Persistence", settings.server.port_tick_persistence),
        ("Bar Persistence", settings.server.port_bar_persistence),
    ];

    println!("{:<20} | {:<10} | {:<10}", "Service", "Port", "Status");
    println!("{:-<20}-|-{:-<10}-|-{:-<10}", "", "", "");

    for (name, port) in services {
        let addr = format!("http://{}:{}", settings.server.host, port);
        let status = match ControlClient::connect(addr).await {
            Ok(mut client) => {
                // Try a simple request to verify health
                match client.list_collections(ListRequest {}).await {
                    Ok(_) => "\x1b[32mHEALTHY\x1b[0m",    // Green
                    Err(_) => "\x1b[31mUNHEALTHY\x1b[0m", // Red
                }
            }
            Err(_) => "\x1b[31mUNHEALTHY\x1b[0m", // Red
        };
        println!("{name:<20} | {port:<10} | {status}");
    }
}

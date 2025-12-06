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

    /// Target service: binance_spot, binance_futures, persistence, timebar_minutes, tibs
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

    let host = if let Some(s) = cli.service {
        let host_ip = &settings.server.host;
        match s.as_str() {
            "binance_spot" => format!("http://{}:{}", host_ip, settings.server.port_spot),
            "binance_futures" => format!("http://{}:{}", host_ip, settings.server.port_futures),
            "persistence" => format!("http://{}:{}", host_ip, settings.server.port_persistence),
            "timebar_minutes" => format!(
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
    }

    Ok(())
}

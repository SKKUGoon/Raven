use clap::{Parser, Subcommand};
use raven::proto::control_client::ControlClient;
use raven::proto::{ControlRequest, ListRequest, StopAllRequest};

#[derive(Parser)]
#[command(name = "ravenctl")]
#[command(about = "Control Raven services", long_about = None)]
struct Cli {
    #[arg(long, default_value = "http://localhost:50051")]
    host: String,

    /// Target service: spot, futures, persistence, bars, tibs
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
    },
    /// Stop data collection for a symbol
    Stop {
        #[arg(short, long)]
        symbol: String,
    },
    /// Stop all data collections
    StopAll,
    /// List active collections
    List,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let host = if let Some(s) = cli.service {
        match s.as_str() {
            "spot" => "http://localhost:50051".to_string(),
            "futures" => "http://localhost:50054".to_string(),
            "persistence" => "http://localhost:50052".to_string(),
            "bars" => "http://localhost:50053".to_string(),
            "tibs" => "http://localhost:50055".to_string(),
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
        Commands::Start { symbol } => {
            let request = ControlRequest {
                symbol,
                exchange: String::new(),
            };
            let response = client.start_collection(request).await?.into_inner();
            println!("Success: {}", response.success);
            println!("Message: {}", response.message);
        }
        Commands::Stop { symbol } => {
            let request = ControlRequest {
                symbol,
                exchange: String::new(),
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

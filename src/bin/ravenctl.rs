use clap::{Parser, Subcommand};
use ptree::TreeBuilder;
use raven::config::Settings;
use raven::proto::control_client::ControlClient;
use raven::proto::{ControlRequest, ListRequest, StopAllRequest};
use std::process::Command;

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
    /// Start services (no args) OR Start data collection for a symbol (with args)
    Start {
        /// Symbol to collect (e.g. BTCUSDT)
        #[arg(short, long)]
        symbol: Option<String>,
        /// Exchange (optional)
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
    /// Stop all data collections (internal)
    StopAll,
    /// Shutdown all Raven services
    Shutdown,
    /// List active collections
    List,
    /// Check status of all services
    Status,
    /// Alias for Status
    Ping,
    /// Show service users/subscriptions tree
    User,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let settings = Settings::new().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {e}. Using defaults.");
        std::process::exit(1);
    });

    // Handle commands that don't need a specific client connection first
    match &cli.command {
        Commands::Status | Commands::Ping => {
            check_status(&settings).await;
            return Ok(());
        }
        Commands::User => {
            show_users_tree(&settings).await;
            return Ok(());
        }
        Commands::Start { symbol, .. } if symbol.is_none() => {
            // "ravenctl start" (no args) -> Start services script
            println!("Starting services using ./start_services.sh...");
            let status = Command::new("sh")
                .arg("start_services.sh")
                .status()
                .expect("Failed to execute start_services.sh");

            if status.success() {
                println!("Services started successfully.");
            } else {
                eprintln!("Failed to start services.");
            }
            return Ok(());
        }
        Commands::Shutdown => {
            println!("Stopping services using ./stop_services.sh...");
            let status = Command::new("sh")
                .arg("stop_services.sh")
                .status()
                .expect("Failed to execute stop_services.sh");

            if status.success() {
                println!("Services stopped successfully.");
            } else {
                eprintln!("Failed to stop services.");
            }
            return Ok(());
        }
        _ => {}
    }

    // For other commands, connect to the target host
    let host = if let Some(s) = cli.service {
        let host_ip = &settings.server.host;
        match s.as_str() {
            "binance_spot" => format!("http://{}:{}", host_ip, settings.server.port_binance_spot),
            "binance_futures" => {
                format!(
                    "http://{}:{}",
                    host_ip, settings.server.port_binance_futures
                )
            }
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
            "timebar" | "timebar_minutes" => {
                format!(
                    "http://{}:{}",
                    host_ip, settings.server.port_timebar_minutes
                )
            }
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
    // We connect lazily or just try to connect now
    let mut client = ControlClient::connect(host).await?;

    match cli.command {
        Commands::Start { symbol, exchange } => {
            // "ravenctl start --symbol ..." -> Start collection
            if let Some(sym) = symbol {
                let request = ControlRequest {
                    symbol: sym,
                    exchange: exchange.unwrap_or_default(),
                };
                let response = client.start_collection(request).await?.into_inner();
                println!("Success: {}", response.success);
                println!("Message: {}", response.message);
            } else {
                // Should have been handled above
                unreachable!();
            }
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
        _ => unreachable!(), // Handled above
    }

    Ok(())
}

async fn check_status(settings: &Settings) {
    let mut services = vec![
        ("Binance Spot", settings.server.port_binance_spot),
        ("Binance Futures", settings.server.port_binance_futures),
        ("TimeBar (1m)", settings.server.port_timebar_minutes),
        ("Tibs", settings.server.port_tibs),
        ("Tick Persistence", settings.server.port_tick_persistence),
        ("Bar Persistence", settings.server.port_bar_persistence),
    ];
    // Manually add the new 1s timebar service (port 50053)
    services.push(("TimeBar (1s)", 50053));

    println!("{:<20} | {:<10} | {:<10}", "Service", "Port", "Status");
    println!("{:-<20}-|-{:-<10}-|-{:-<10}", "", "", "");

    for (name, port) in services {
        let addr = format!("http://{}:{}", settings.server.host, port);
        let status = match ControlClient::connect(addr).await {
            Ok(mut client) => {
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

async fn show_users_tree(settings: &Settings) {
    let mut services = vec![
        ("Binance Spot", settings.server.port_binance_spot),
        ("Binance Futures", settings.server.port_binance_futures),
        ("TimeBar (1m)", settings.server.port_timebar_minutes),
        ("Tibs", settings.server.port_tibs),
        ("Tick Persistence", settings.server.port_tick_persistence),
        ("Bar Persistence", settings.server.port_bar_persistence),
    ];
    // Manually add the new 1s timebar service (port 50053)
    services.push(("TimeBar (1s)", 50053));

    let mut tree = TreeBuilder::new("Raven Cluster".to_string());

    for (name, port) in services {
        let addr = format!("http://{}:{}", settings.server.host, port);
        let status = match ControlClient::connect(addr).await {
            Ok(mut client) => match client.list_collections(ListRequest {}).await {
                Ok(resp) => {
                    let collections = resp.into_inner().collections;
                    let node_text = format!("{} ({} active)", name, collections.len());
                    let service_node = tree.begin_child(node_text);

                    for c in collections {
                        let info = format!("{} [subs: {}]", c.symbol, c.subscriber_count);
                        service_node.add_empty_child(info);
                    }
                    service_node.end_child();
                    true
                }
                Err(_) => false,
            },
            Err(_) => false,
        };

        if !status {
            // Service is down, maybe show it?
            let node_text = format!("{name} (UNREACHABLE)");
            tree.add_empty_child(node_text);
        }
    }

    let _ = ptree::print_tree(&tree.build());
}

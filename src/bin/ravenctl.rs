use clap::{Parser, Subcommand};
use raven::config::Settings;
use raven::domain::instrument::{Asset, Instrument};
use raven::domain::venue::VenueId;
use raven::proto::control_client::ControlClient;
use raven::proto::{ControlRequest, DataType, ListRequest, StopAllRequest};
use raven::routing::symbol_resolver::SymbolResolver;
use raven::routing::venue_selector::VenueSelector;
use raven::utils::process::{start_all_services_with_settings, stop_all_services};
use raven::utils::service_registry;
use raven::utils::status::check_status;
use raven::utils::tree::show_users_tree;
use raven::utils::grpc::wait_for_control_ready;
use std::io::{Error as IoError, ErrorKind};

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
        /// Symbol or base asset to collect.
        ///
        /// - If you pass `--base`, this is interpreted as the base asset (e.g. ETH).
        /// - If you omit `--base`, this is interpreted as a venue symbol (e.g. ETHUSDC).
        #[arg(short, long)]
        symbol: Option<String>,
        /// Quote / base currency (e.g. USDC). If provided, `--symbol` is treated as the base asset.
        #[arg(long)]
        base: Option<String>,
        /// Target venue (single-venue selector).
        #[arg(short, long)]
        exchange: Option<String>,
        /// Optional allowlist of venues (may be repeated). Overrides config allowlist.
        #[arg(long = "venue-include")]
        venue_include: Vec<String>,
        /// Optional denylist of venues (may be repeated). Overrides config denylist.
        #[arg(long = "venue-exclude")]
        venue_exclude: Vec<String>,
    },
    /// Stop data collection for a symbol
    Stop {
        /// Symbol or base asset to stop.
        ///
        /// - If you pass `--base`, this is interpreted as the base asset (e.g. ETH).
        /// - If you omit `--base`, this is interpreted as a venue symbol (e.g. ETHUSDC).
        #[arg(short, long)]
        symbol: String,
        /// Quote / base currency (e.g. USDC). If provided, `--symbol` is treated as the base asset.
        #[arg(long)]
        base: Option<String>,
        /// Target venue (single-venue selector).
        #[arg(short, long)]
        venue: Option<String>,
        /// Optional allowlist of venues (may be repeated). Overrides config allowlist.
        #[arg(long = "venue-include")]
        venue_include: Vec<String>,
        /// Optional denylist of venues (may be repeated). Overrides config denylist.
        #[arg(long = "venue-exclude")]
        venue_exclude: Vec<String>,
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

use std::time::Duration;

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
        Commands::Start {
            symbol,
            base,
            exchange,
            venue_include,
            venue_exclude,
        } => {
            // Case 1: No symbol provided -> Start Infrastructure ONLY
            if symbol.is_none() {
                start_all_services_with_settings(&settings);
                return Ok(());
            }

            // Case 2: Symbol provided -> Start Infrastructure AND Start Collection
            // First, ensure services are running
            start_all_services_with_settings(&settings);

            let sym_raw = symbol.as_ref().unwrap();

            // Build instrument if base is provided; otherwise treat --symbol as a venue symbol.
            let instrument = match &base {
                Some(base_raw) => {
                    let base_asset: Asset = sym_raw
                        .parse()
                        .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?;
                    let quote_asset: Asset = base_raw
                        .parse()
                        .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?;
                    Some(Instrument::new(base_asset, quote_asset))
                }
                None => None,
            };

            let resolver = SymbolResolver::from_config(&settings.routing);

            // Resolve venues:
            // - if --exchange provided, we run only that venue (backwards-compat)
            // - else: merge config selector + CLI overrides
            let venues: Vec<VenueId> = if let Some(exch) = exchange.as_deref() {
                vec![exch
                    .parse()
                    .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?]
            } else {
                let selector = VenueSelector {
                    include: if venue_include.is_empty() {
                        settings.routing.venue_include.clone()
                    } else {
                        venue_include.clone()
                    },
                    exclude: if venue_exclude.is_empty() {
                        settings.routing.venue_exclude.clone()
                    } else {
                        venue_exclude.clone()
                    },
                };
                selector.resolve()
            };

            if venues.is_empty() {
                eprintln!("No venues selected (after include/exclude). Nothing to do.");
                return Ok(());
            }

            let tick_host = format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_tick_persistence
            );
            let bar_host = format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_bar_persistence
            );
            let timebar_host = format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_timebar_minutes
            );
            let tibs_host = format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_tibs
            );

            // Readiness checks
            let ready_timeout = Duration::from_secs(15);
            for (name, host) in [
                ("tick_persistence", tick_host.clone()),
                ("bar_persistence", bar_host.clone()),
                ("timebar", timebar_host.clone()),
                ("tibs", tibs_host.clone()),
            ] {
                if !wait_for_control_ready(&host, ready_timeout).await {
                    eprintln!("Service {name} not ready at {host} (timeout {ready_timeout:?})");
                }
            }

            for venue in venues {
                let venue_wire = venue.as_wire();

                let venue_symbol = match &instrument {
                    Some(instr) => resolver.resolve(instr, &venue),
                    None => sym_raw.clone(),
                };

                println!(
                    "Starting collection pipeline for {} on {} (venue_symbol={})...",
                    instrument
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| sym_raw.clone()),
                    venue_wire,
                    venue_symbol
                );

                // 1. Start Collector (e.g. binance_spot)
                let collector_port = match venue {
                    VenueId::BinanceFutures => settings.server.port_binance_futures,
                    _ => settings.server.port_binance_spot,
                };
                let collector_host = format!("http://{}:{}", settings.server.host, collector_port);

                if !wait_for_control_ready(&collector_host, ready_timeout).await {
                    eprintln!(
                        "Collector not ready at {collector_host} (timeout {ready_timeout:?})"
                    );
                }

                // Wire downstream FIRST
                // 1) Tick Persistence (TRADE + ORDERBOOK)
                match ControlClient::connect(tick_host.clone()).await {
                    Ok(mut client) => {
                        // Trades
                        let req_trade = ControlRequest {
                            symbol: venue_symbol.clone(),
                            venue: venue_wire.clone(),
                            data_type: DataType::Trade as i32,
                        };
                        let _ = client.start_collection(req_trade).await;

                        // Orderbook
                        let req_book = ControlRequest {
                            symbol: venue_symbol.clone(),
                            venue: venue_wire.clone(),
                            data_type: DataType::Orderbook as i32,
                        };
                        let _ = client.start_collection(req_book).await;

                        println!("  [+] Tick Persistence started for {venue_symbol}");
                    }
                    Err(e) => eprintln!("  [-] Failed to connect to tick persistence: {e}"),
                }

                // 2) Bar Persistence (CANDLE)
                match ControlClient::connect(bar_host.clone()).await {
                    Ok(mut client) => {
                        let req = ControlRequest {
                            symbol: venue_symbol.clone(),
                            venue: venue_wire.clone(),
                            data_type: DataType::Candle as i32,
                        };
                        let _ = client.start_collection(req).await;
                        println!("  [+] Bar Persistence started for {venue_symbol}");
                    }
                    Err(e) => eprintln!("  [-] Failed to connect to bar persistence: {e}"),
                }

                // 3) Aggregators (Timebars) - output is CANDLE
                match ControlClient::connect(timebar_host.clone()).await {
                    Ok(mut client) => {
                        let req = ControlRequest {
                            symbol: venue_symbol.clone(),
                            venue: venue_wire.clone(),
                            data_type: DataType::Candle as i32,
                        };
                        let _ = client.start_collection(req).await;
                        println!("  [+] Timebar started for {venue_symbol}");
                    }
                    Err(e) => eprintln!("  [-] Failed to connect to timebar: {e}"),
                }

                // 4) Aggregators (Tibs) - output is CANDLE
                match ControlClient::connect(tibs_host.clone()).await {
                    Ok(mut client) => {
                        let req = ControlRequest {
                            symbol: venue_symbol.clone(),
                            venue: venue_wire.clone(),
                            data_type: DataType::Candle as i32,
                        };
                        let _ = client.start_collection(req).await;
                        println!("  [+] Tibs started for {venue_symbol}");
                    }
                    Err(e) => eprintln!("  [-] Failed to connect to tibs: {e}"),
                }

                // Start upstream LAST (this is when the actual venue WS subscription happens)
                match ControlClient::connect(collector_host.clone()).await {
                    Ok(mut client) => {
                        // Trades
                        let req_trade = ControlRequest {
                            symbol: venue_symbol.clone(),
                            venue: venue_wire.clone(),
                            data_type: DataType::Trade as i32,
                        };
                        let _ = client.start_collection(req_trade).await;

                        // Orderbook
                        let req_book = ControlRequest {
                            symbol: venue_symbol.clone(),
                            venue: venue_wire.clone(),
                            data_type: DataType::Orderbook as i32,
                        };
                        let _ = client.start_collection(req_book).await;

                        println!("  [+] Collector started for {venue_symbol}");
                    }
                    Err(e) => eprintln!("  [-] Failed to connect to collector: {e}"),
                }
            }

            return Ok(());
        }
        Commands::Shutdown => {
            stop_all_services();
            return Ok(());
        }
        _ => {}
    }

    // For other commands (Stop, StopAll, List), connect to the target host
    let host = if let Some(s) = cli.service {
        let host_ip = &settings.server.host;
        // Resolve against our canonical service registry first.
        let services = service_registry::all_services(&settings);
        if let Some(spec) = services.iter().find(|svc| svc.id == s.as_str()) {
            spec.addr(host_ip)
        } else {
            // Backwards-compatible aliases
            match s.as_str() {
                "persistence" => format!(
                    "http://{}:{}",
                    host_ip, settings.server.port_tick_persistence
                ),
                "timebar" | "timebar_minutes" => format!(
                    "http://{}:{}",
                    host_ip, settings.server.port_timebar_minutes
                ),
                "timebar_seconds" => format!(
                    "http://{}:{}",
                    host_ip, settings.server.port_timebar_seconds
                ),
                _ => {
                    eprintln!("Unknown service: {s}. Using default host.");
                    cli.host
                }
            }
        }
    } else {
        cli.host
    };

    println!("Connecting to {host}");
    // We connect lazily or just try to connect now
    let mut client = ControlClient::connect(host).await?;

    match cli.command {
        // Start is handled above
        Commands::Start { .. } => unreachable!(),

        Commands::Stop { symbol, base, venue, venue_include, venue_exclude } => {
            // Build instrument if base is provided; otherwise treat --symbol as a venue symbol.
            let instrument = match &base {
                Some(base_raw) => {
                    let base_asset: Asset = symbol
                        .parse()
                        .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?;
                    let quote_asset: Asset = base_raw
                        .parse()
                        .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?;
                    Some(Instrument::new(base_asset, quote_asset))
                }
                None => None,
            };

            let resolver = SymbolResolver::from_config(&settings.routing);
            let venues: Vec<VenueId> = if let Some(v) = venue.as_deref() {
                vec![v
                    .parse()
                    .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?]
            } else {
                let selector = VenueSelector {
                    include: if venue_include.is_empty() {
                        settings.routing.venue_include.clone()
                    } else {
                        venue_include
                    },
                    exclude: if venue_exclude.is_empty() {
                        settings.routing.venue_exclude.clone()
                    } else {
                        venue_exclude
                    },
                };
                selector.resolve()
            };

            if venues.is_empty() {
                eprintln!("No venues selected (after include/exclude). Nothing to do.");
                return Ok(());
            }

            // Service hosts
            let tick_host = format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_tick_persistence
            );
            let bar_host = format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_bar_persistence
            );
            let timebar_host = format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_timebar_minutes
            );
            let tibs_host = format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_tibs
            );

            for venue_id in venues {
                let venue_wire = venue_id.as_wire();
                let venue_symbol = match &instrument {
                    Some(instr) => resolver.resolve(instr, &venue_id),
                    None => symbol.clone(),
                };

                // Downstream first
                for (name, host, data_types) in [
                    ("tick_persistence", tick_host.clone(), vec![DataType::Trade, DataType::Orderbook]),
                    ("bar_persistence", bar_host.clone(), vec![DataType::Candle]),
                    ("timebar", timebar_host.clone(), vec![DataType::Candle]),
                    ("tibs", tibs_host.clone(), vec![DataType::Candle]),
                ] {
                    match ControlClient::connect(host.clone()).await {
                        Ok(mut svc) => {
                            for dt in data_types {
                                let req = ControlRequest {
                                    symbol: venue_symbol.clone(),
                                    venue: venue_wire.clone(),
                                    data_type: dt as i32,
                                };
                                let _ = svc.stop_collection(req).await;
                            }
                            println!("  [-] Stopped {name} for {venue_symbol} ({venue_wire})");
                        }
                        Err(e) => eprintln!("  [!] Failed to connect to {name}: {e}"),
                    }
                }

                // Upstream last
                let collector_port = match venue_id {
                    VenueId::BinanceFutures => settings.server.port_binance_futures,
                    _ => settings.server.port_binance_spot,
                };
                let collector_host = format!("http://{}:{}", settings.server.host, collector_port);
                match ControlClient::connect(collector_host.clone()).await {
                    Ok(mut svc) => {
                        for dt in [DataType::Trade, DataType::Orderbook] {
                            let req = ControlRequest {
                                symbol: venue_symbol.clone(),
                                venue: venue_wire.clone(),
                                data_type: dt as i32,
                            };
                            let _ = svc.stop_collection(req).await;
                        }
                        println!("  [-] Stopped collector for {venue_symbol} ({venue_wire})");
                    }
                    Err(e) => eprintln!("  [!] Failed to connect to collector: {e}"),
                }
            }
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

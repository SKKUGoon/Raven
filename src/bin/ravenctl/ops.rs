use raven::config::Settings;
use raven::domain::instrument::{Asset, Instrument};
use raven::domain::venue::VenueId;
use raven::proto::control_client::ControlClient;
use raven::proto::{ControlRequest, DataType, StopAllRequest};
use raven::routing::symbol_resolver::SymbolResolver;
use raven::routing::venue_selector::VenueSelector;
use raven::utils::grpc::wait_for_control_ready;
use raven::utils::process::{start_all_services_with_settings, stop_all_services, stop_service};
use raven::utils::service_registry;
use std::io::{Error as IoError, ErrorKind};
use std::time::Duration;

#[derive(Clone)]
struct ServiceHosts {
    tick: String,
    bar: String,
    timebar: String,
    tibs: String,
}

impl ServiceHosts {
    fn from_settings(settings: &Settings) -> Self {
        Self {
            tick: format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_tick_persistence
            ),
            bar: format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_bar_persistence
            ),
            timebar: format!(
                "http://{}:{}",
                settings.server.host, settings.server.port_timebar_minutes
            ),
            tibs: format!("http://{}:{}", settings.server.host, settings.server.port_tibs),
        }
    }
}

fn parse_asset(s: &str) -> Result<Asset, IoError> {
    s.parse()
        .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))
}

fn parse_venue(s: &str) -> Result<VenueId, IoError> {
    s.parse()
        .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))
}

fn build_instrument(symbol_or_base: &str, base: &Option<String>) -> Result<Option<Instrument>, IoError> {
    match base {
        Some(quote_raw) => {
            let base_asset = parse_asset(symbol_or_base)?;
            let quote_asset = parse_asset(quote_raw)?;
            Ok(Some(Instrument::new(base_asset, quote_asset)))
        }
        None => Ok(None),
    }
}

fn resolve_venues(
    settings: &Settings,
    // If Some, this overrides selector for backwards compat.
    single_venue: Option<&str>,
    venue_include: &[String],
    venue_exclude: &[String],
) -> Result<Vec<VenueId>, IoError> {
    if let Some(v) = single_venue {
        return Ok(vec![parse_venue(v)?]);
    }

    let selector = VenueSelector {
        include: if venue_include.is_empty() {
            settings.routing.venue_include.clone()
        } else {
            venue_include.to_vec()
        },
        exclude: if venue_exclude.is_empty() {
            settings.routing.venue_exclude.clone()
        } else {
            venue_exclude.to_vec()
        },
    };
    Ok(selector.resolve())
}

pub async fn stop_all_collections_cluster(settings: &Settings) {
    let host_ip = service_registry::client_host(&settings.server.host);
    let services = service_registry::all_services(settings);

    println!("Stopping all collections across {} services...", services.len());
    for svc in services {
        let addr = svc.addr(host_ip);
        match ControlClient::connect(addr.clone()).await {
            Ok(mut client) => match client.stop_all_collections(StopAllRequest {}).await {
                Ok(resp) => {
                    let inner = resp.into_inner();
                    println!(
                        "- {} ({}) @ {} -> success={} msg={}",
                        svc.display_name, svc.id, addr, inner.success, inner.message
                    );
                }
                Err(e) => println!("- {} ({}) @ {} -> ERROR: {e}", svc.display_name, svc.id, addr),
            },
            Err(e) => println!(
                "- {} ({}) @ {} -> UNREACHABLE: {e}",
                svc.display_name, svc.id, addr
            ),
        }
    }
}

pub fn shutdown(settings: &Settings, service_opt: &Option<String>) {
    if let Some(svc_id) = service_opt.as_deref() {
        let services = service_registry::all_services(settings);
        if let Some(spec) = services.iter().find(|svc| svc.id == svc_id) {
            if stop_service(spec.log_name) {
                println!("Shutdown requested for {}", spec.display_name);
            } else {
                eprintln!(
                    "No pid file found for {} ({}). Did you start it via `ravenctl start`?",
                    spec.display_name, spec.log_name
                );
            }
        } else {
            eprintln!("Unknown service id: {svc_id}");
        }
    } else {
        stop_all_services();
    }
}

pub fn resolve_control_host(cli_host: String, service_opt: &Option<String>, settings: &Settings) -> String {
    if let Some(s) = service_opt.as_deref() {
        let host_ip = &settings.server.host;
        // Resolve against our canonical service registry first.
        let services = service_registry::all_services(settings);
        if let Some(spec) = services.iter().find(|svc| svc.id == s) {
            spec.addr(host_ip)
        } else {
            // Backwards-compatible aliases
            match s {
                "persistence" => format!("http://{}:{}", host_ip, settings.server.port_tick_persistence),
                "timebar" | "timebar_minutes" => {
                    format!("http://{}:{}", host_ip, settings.server.port_timebar_minutes)
                }
                "timebar_seconds" => {
                    format!("http://{}:{}", host_ip, settings.server.port_timebar_seconds)
                }
                _ => {
                    eprintln!("Unknown service: {s}. Using default host.");
                    cli_host
                }
            }
        }
    } else {
        cli_host
    }
}

pub async fn handle_start(
    settings: &Settings,
    symbol: &Option<String>,
    base: &Option<String>,
    exchange: &Option<String>,
    venue_include: &[String],
    venue_exclude: &[String],
) -> Result<(), IoError> {
    // Case 1: No symbol provided -> Start Infrastructure ONLY
    if symbol.is_none() {
        start_all_services_with_settings(settings);
        return Ok(());
    }

    // Case 2: Symbol provided -> Start Infrastructure AND Start Collection
    // First, ensure services are running
    start_all_services_with_settings(settings);

    let sym_raw = symbol.as_ref().unwrap();

    // Build instrument if base is provided; otherwise treat --symbol as a venue symbol.
    let instrument = build_instrument(sym_raw, base)?;

    let resolver = SymbolResolver::from_config(&settings.routing);

    // Resolve venues:
    // - if --exchange provided, we run only that venue (backwards-compat)
    // - else: merge config selector + CLI overrides
    let venues = resolve_venues(
        settings,
        exchange.as_deref(),
        venue_include,
        venue_exclude,
    )?;

    if venues.is_empty() {
        eprintln!("No venues selected (after include/exclude). Nothing to do.");
        return Ok(());
    }

    let hosts = ServiceHosts::from_settings(settings);

    // Readiness checks
    let ready_timeout = Duration::from_secs(15);
    for (name, host) in [
        ("tick_persistence", hosts.tick.clone()),
        ("bar_persistence", hosts.bar.clone()),
        ("timebar", hosts.timebar.clone()),
        ("tibs", hosts.tibs.clone()),
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
            eprintln!("Collector not ready at {collector_host} (timeout {ready_timeout:?})");
        }

        // Wire downstream FIRST
        // 1) Tick Persistence (TRADE + ORDERBOOK)
        match ControlClient::connect(hosts.tick.clone()).await {
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
        match ControlClient::connect(hosts.bar.clone()).await {
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
        match ControlClient::connect(hosts.timebar.clone()).await {
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
        match ControlClient::connect(hosts.tibs.clone()).await {
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

    Ok(())
}

pub async fn handle_stop(
    settings: &Settings,
    symbol: String,
    base: Option<String>,
    venue: Option<String>,
    venue_include: Vec<String>,
    venue_exclude: Vec<String>,
) -> Result<(), IoError> {
    // Build instrument if base is provided; otherwise treat --symbol as a venue symbol.
    let instrument = build_instrument(&symbol, &base)?;

    let resolver = SymbolResolver::from_config(&settings.routing);
    let venues = resolve_venues(
        settings,
        venue.as_deref(),
        &venue_include,
        &venue_exclude,
    )?;

    if venues.is_empty() {
        eprintln!("No venues selected (after include/exclude). Nothing to do.");
        return Ok(());
    }

    let hosts = ServiceHosts::from_settings(settings);

    for venue_id in venues {
        let venue_wire = venue_id.as_wire();
        let venue_symbol = match &instrument {
            Some(instr) => resolver.resolve(instr, &venue_id),
            None => symbol.clone(),
        };

        // Downstream first
        for (name, host, data_types) in [
            (
                "tick_persistence",
                hosts.tick.clone(),
                vec![DataType::Trade, DataType::Orderbook],
            ),
            ("bar_persistence", hosts.bar.clone(), vec![DataType::Candle]),
            ("timebar", hosts.timebar.clone(), vec![DataType::Candle]),
            ("tibs", hosts.tibs.clone(), vec![DataType::Candle]),
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

    Ok(())
}



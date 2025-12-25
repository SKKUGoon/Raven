use raven::config::Settings;
use raven::pipeline::spec::PipelineSpec;
use raven::routing::symbol_resolver::SymbolResolver;
use raven::utils::grpc::wait_for_control_ready;
use raven::utils::process::start_all_services_with_settings;
use std::io::{Error as IoError, ErrorKind};
use std::time::Duration;

use super::util::{build_instrument, resolve_venues, service_addr, start_stream, stop_stream};

pub async fn handle_start(
    settings: &Settings,
    symbol: &Option<String>,
    base: &Option<String>,
    venue: &Option<String>,
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
    // - if --venue provided, we run only that venue
    // - else: merge config selector + CLI overrides
    let venues = resolve_venues(settings, venue.as_deref(), venue_include, venue_exclude)?;

    if venues.is_empty() {
        eprintln!("No venues selected (after include/exclude). Nothing to do.");
        return Ok(());
    }

    let pipeline = PipelineSpec::default();
    let ready_timeout = Duration::from_secs(15);

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

        let order = pipeline
            .start_order_for_venue(&venue)
            .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?;

        // Readiness checks for the nodes we will touch (including the venue-specific collector).
        for node_id in &order {
            if let Some(addr) = service_addr(settings, node_id.service_id()) {
                if !wait_for_control_ready(&addr, ready_timeout).await {
                    eprintln!(
                        "Service {} not ready at {} (timeout {:?})",
                        node_id.label(),
                        addr,
                        ready_timeout
                    );
                }
            } else {
                eprintln!("Unknown service id in pipeline: {}", node_id.service_id());
            }
        }

        // Execute start plan (downstream-first, collector-last).
        for node_id in order {
            let addr = match service_addr(settings, node_id.service_id()) {
                Some(a) => a,
                None => {
                    eprintln!("Unknown service id in pipeline: {}", node_id.service_id());
                    continue;
                }
            };

            // Streams are derived from the PipelineSpec graph (edges + node kind).
            for stream in pipeline.required_streams_for_node(node_id, &venue) {
                start_stream(&addr, &venue_symbol, &venue_wire, stream.as_proto_i32()).await;
            }

            println!("  [+] {} started for {}", node_id.label(), venue_symbol);
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
    let venues = resolve_venues(settings, venue.as_deref(), &venue_include, &venue_exclude)?;

    if venues.is_empty() {
        eprintln!("No venues selected (after include/exclude). Nothing to do.");
        return Ok(());
    }

    let pipeline = PipelineSpec::default();

    for venue_id in venues {
        let venue_wire = venue_id.as_wire();
        let venue_symbol = match &instrument {
            Some(instr) => resolver.resolve(instr, &venue_id),
            None => symbol.clone(),
        };

        println!(
            "Stopping collection pipeline for {} on {} (venue_symbol={})...",
            instrument
                .as_ref()
                .map(|i| i.to_string())
                .unwrap_or_else(|| symbol.clone()),
            venue_wire,
            venue_symbol
        );

        // Stop should be upstream-first, downstream-last.
        let order = pipeline
            .stop_order_for_venue(&venue_id)
            .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?;

        for node_id in order {
            let addr = match service_addr(settings, node_id.service_id()) {
                Some(a) => a,
                None => {
                    eprintln!("Unknown service id in pipeline: {}", node_id.service_id());
                    continue;
                }
            };
            // Streams are derived from the PipelineSpec graph (edges + node kind).
            for stream in pipeline.required_streams_for_node(node_id, &venue_id) {
                stop_stream(&addr, &venue_symbol, &venue_wire, stream.as_proto_i32()).await;
            }
            println!("  [-] {} stopped for {}", node_id.label(), venue_symbol);
        }
    }

    Ok(())
}



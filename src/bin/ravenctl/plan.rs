use raven::config::Settings;
use raven::domain::venue::VenueId;
use raven::pipeline::spec::PipelineSpec;
use raven::routing::symbol_resolver::SymbolResolver;
use std::io::{Error as IoError, ErrorKind};

use super::util::{build_instrument, resolve_venues, service_addr};

pub async fn handle_plan(
    settings: &Settings,
    coin: &str,
    quote: &Option<String>,
    venue: &Option<String>,
    venue_include: &[String],
    venue_exclude: &[String],
) -> Result<String, IoError> {
    let instrument = build_instrument(coin, quote)?;
    let resolver = SymbolResolver::from_config(&settings.routing);
    let venues = resolve_venues(settings, venue.as_deref(), venue_include, venue_exclude)?;

    if venues.is_empty() {
        return Ok("No venues selected (after include/exclude). Nothing to do.\n".to_string());
    }

    let pipeline = PipelineSpec::default();
    let mut out = String::new();

    for venue in venues {
        let venue_wire = venue.as_wire();
        let venue_symbol = match &instrument {
            Some(instr) => resolver.resolve(instr, &venue),
            None => coin.to_string(),
        };

        out.push_str(&format!(
            "Plan: start pipeline for {} on {} (venue_symbol={})\n",
            instrument
                .as_ref()
                .map(|i| i.to_string())
                .unwrap_or_else(|| coin.to_string()),
            venue_wire,
            venue_symbol
        ));

        let order = pipeline
            .start_order_for_venue(&venue)
            .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))?;

        for node_id in order {
            let addr = match service_addr(settings, node_id.service_id()) {
                Some(a) => a,
                None => {
                    out.push_str(&format!(
                        "  - {} (service_id={}) -> ERROR: unknown service id\n",
                        node_id.label(),
                        node_id.service_id()
                    ));
                    continue;
                }
            };

            // Derived from the PipelineSpec graph (edges + node kind).
            out.push_str(&format!("  - {} @ {}\n", node_id.label(), addr));
            for stream in pipeline.required_streams_for_node(node_id, &venue) {
                out.push_str(&format!(
                    "    start_collection(symbol={}, venue={}, data_type={})\n",
                    venue_symbol,
                    venue_wire,
                    stream.label()
                ));
            }
        }
        let process_services = process_services_for_venue(&venue);
        out.push_str("  process services in scope:\n");
        for svc in process_services {
            out.push_str(&format!("    - {svc}\n"));
        }
        out.push('\n');
    }

    Ok(out)
}

fn process_services_for_venue(venue: &VenueId) -> Vec<&'static str> {
    match venue {
        VenueId::BinanceSpot => vec![
            "binance_spot",
            "tick_persistence",
            "tibs_small",
            "tibs_large",
            "trbs_small",
            "trbs_large",
            "vibs_small",
            "vibs_large",
            "vpin",
            "bar_persistence",
        ],
        VenueId::BinanceFutures => vec![
            "binance_futures",
            "binance_futures_funding",
            "binance_futures_liquidations",
            "binance_futures_oi",
            "tick_persistence",
            "tibs_small",
            "tibs_large",
            "trbs_small",
            "trbs_large",
            "vibs_small",
            "vibs_large",
            "vpin",
            "bar_persistence",
        ],
        VenueId::Other(v) if v == "BINANCE_OPTIONS" => {
            vec!["binance_options", "tick_persistence"]
        }
        VenueId::Other(v) if v == "DERIBIT" => vec![
            "deribit_option",
            "deribit_trades",
            "deribit_index",
            "tick_persistence",
        ],
        VenueId::Other(_) => vec![],
    }
}

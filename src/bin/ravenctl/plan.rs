use raven::config::Settings;
use raven::pipeline::spec::PipelineSpec;
use raven::routing::symbol_resolver::SymbolResolver;
use std::io::{Error as IoError, ErrorKind};

use super::util::{build_instrument, resolve_venues, service_addr};

pub async fn handle_plan(
    settings: &Settings,
    symbol: &str,
    base: &Option<String>,
    venue: &Option<String>,
    venue_include: &[String],
    venue_exclude: &[String],
) -> Result<String, IoError> {
    let instrument = build_instrument(symbol, base)?;
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
            None => symbol.to_string(),
        };

        out.push_str(&format!(
            "Plan: start pipeline for {} on {} (venue_symbol={})\n",
            instrument
                .as_ref()
                .map(|i| i.to_string())
                .unwrap_or_else(|| symbol.to_string()),
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
        out.push('\n');
    }

    Ok(out)
}



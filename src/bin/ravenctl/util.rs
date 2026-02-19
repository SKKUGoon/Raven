use raven::config::Settings;
use raven::domain::asset::Asset;
use raven::domain::instrument::Instrument;
use raven::domain::venue::VenueId;
use raven::proto::control_client::ControlClient;
use raven::proto::ControlRequest;
use raven::routing::venue_selector::VenueSelector;
use raven::utils::service_registry;
use std::io::{Error as IoError, ErrorKind};

pub(super) fn service_addr(settings: &Settings, service_id: &str) -> Option<String> {
    let host_ip = service_registry::client_host(&settings.server.host);
    service_registry::all_services(settings)
        .into_iter()
        .find(|s| s.id == service_id)
        .map(|s| s.addr(host_ip))
}

pub(super) async fn start_stream(addr: &str, venue_symbol: &str, venue_wire: &str, data_type: i32) {
    match ControlClient::connect(addr.to_string()).await {
        Ok(mut client) => {
            let req = ControlRequest {
                symbol: venue_symbol.to_string(),
                venue: venue_wire.to_string(),
                data_type,
            };
            let _ = client.start_collection(req).await;
        }
        Err(e) => eprintln!("  [-] Failed to connect to {addr}: {e}"),
    }
}

pub(super) async fn stop_stream(addr: &str, venue_symbol: &str, venue_wire: &str, data_type: i32) {
    match ControlClient::connect(addr.to_string()).await {
        Ok(mut client) => {
            let req = ControlRequest {
                symbol: venue_symbol.to_string(),
                venue: venue_wire.to_string(),
                data_type,
            };
            let _ = client.stop_collection(req).await;
        }
        Err(e) => eprintln!("  [!] Failed to connect to {addr}: {e}"),
    }
}

fn parse_asset(s: &str) -> Result<Asset, IoError> {
    s.parse()
        .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))
}

pub(super) fn parse_venue(s: &str) -> Result<VenueId, IoError> {
    s.parse()
        .map_err(|e| IoError::new(ErrorKind::InvalidInput, e))
}

pub(super) fn build_instrument(
    coin_or_venue_symbol: &str,
    quote: &Option<String>,
) -> Result<Option<Instrument>, IoError> {
    match quote {
        Some(quote_raw) => {
            let coin_asset = parse_asset(coin_or_venue_symbol)?;
            let quote_asset = parse_asset(quote_raw)?;
            Ok(Some(Instrument::new(coin_asset, quote_asset)))
        }
        None => Ok(None),
    }
}

pub(super) fn resolve_venues(
    settings: &Settings,
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

use std::collections::HashMap;

use crate::config::RoutingConfig;
use crate::domain::instrument::Instrument;
use crate::domain::venue::VenueId;

#[derive(Clone, Debug)]
pub struct SymbolResolver {
    /// Map from canonical instrument string (e.g. "PEPE/USDT") to venue->symbol.
    /// Stored as strings to keep config formats simple and stable.
    map: HashMap<String, HashMap<String, String>>,
}

impl SymbolResolver {
    pub fn from_config(cfg: &RoutingConfig) -> Self {
        Self {
            map: cfg.symbol_map.clone(),
        }
    }

    /// Resolve the venue-specific symbol to use for upstream subscriptions/control.
    /// Falls back to `instrument.default_venue_symbol()` if no override exists.
    pub fn resolve(&self, instrument: &Instrument, venue: &VenueId) -> String {
        let key = instrument.canonical();
        let venue_key = venue.as_wire();

        self.map
            .get(&key)
            .and_then(|m| m.get(&venue_key))
            .cloned()
            .unwrap_or_else(|| default_symbol_for_venue(instrument, &venue_key))
    }
}

fn default_symbol_for_venue(instrument: &Instrument, venue_wire: &str) -> String {
    if venue_wire.eq_ignore_ascii_case("DERIBIT") {
        instrument.deribit_symbol()
    } else {
        instrument.binance_symbol()
    }
}

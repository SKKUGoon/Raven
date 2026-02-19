use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VenueId {
    BinanceSpot,
    BinanceFutures,
    Other(String),
}

impl VenueId {
    pub fn as_wire(&self) -> String {
        match self {
            VenueId::BinanceSpot => "BINANCE_SPOT".to_string(),
            VenueId::BinanceFutures => "BINANCE_FUTURES".to_string(),
            VenueId::Other(v) => v.clone(),
        }
    }
}

impl fmt::Display for VenueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_wire())
    }
}

impl FromStr for VenueId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        if t.is_empty() {
            return Err("venue is empty");
        }
        let u = t.to_uppercase();
        Ok(match u.as_str() {
            "BINANCE_SPOT" | "BINANCE" => VenueId::BinanceSpot,
            "BINANCE_FUTURES" | "BINANCE_FUTURE" | "BINANCE_PERP" => VenueId::BinanceFutures,
            _ => VenueId::Other(u),
        })
    }
}

pub fn default_venues() -> Vec<VenueId> {
    vec![
        VenueId::BinanceSpot,
        VenueId::BinanceFutures,
        VenueId::Other("DERIBIT".to_string()),
    ]
}

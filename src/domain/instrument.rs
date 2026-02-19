use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::asset::Asset;

/// Canonical instrument representation inside Raven.
/// This is deliberately venue-agnostic.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instrument {
    pub symbol: Asset,
    pub quote: Asset,
}

impl Instrument {
    pub fn new(symbol: Asset, quote: Asset) -> Self {
        Self { symbol, quote }
    }

    /// Binance-style venue symbol (e.g. BTCUSDT).
    pub fn binance_symbol(&self) -> String {
        format!("{}{}", self.symbol, self.quote)
    }

    /// Deribit-style venue symbol (e.g. btc_usd).
    pub fn deribit_symbol(&self) -> String {
        format!(
            "{}_{}",
            self.symbol.as_str().to_lowercase(),
            self.quote.as_str().to_lowercase()
        )
    }

    /// Default venue symbol, historically used by Raven services.
    pub fn default_venue_symbol(&self) -> String {
        self.binance_symbol()
    }

    /// Human-readable canonical form (e.g. BTC/USDT).
    pub fn canonical(&self) -> String {
        format!("{}/{}", self.symbol, self.quote)
    }
}

impl fmt::Display for Instrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.symbol, self.quote)
    }
}

impl FromStr for Instrument {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        let (base, quote) = t.split_once('/').ok_or("instrument must be BASE/QUOTE")?;
        let base: Asset = base.parse()?;
        let quote: Asset = quote.parse()?;
        Ok(Instrument::new(base, quote))
    }
}

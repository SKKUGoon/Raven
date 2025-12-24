use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Asset(String);

impl Asset {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Asset {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        if t.is_empty() {
            return Err("asset is empty");
        }
        // Keep it simple: normalize to uppercase and reject whitespace.
        if t.chars().any(|c| c.is_whitespace()) {
            return Err("asset contains whitespace");
        }
        Ok(Self(t.to_uppercase()))
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

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

    /// Default venue symbol, historically used by Raven services (e.g. BTCUSDT).
    pub fn default_venue_symbol(&self) -> String {
        format!("{}{}", self.symbol, self.quote)
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

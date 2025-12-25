use std::fmt;

use crate::proto::DataType;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StreamDataType {
    Trade,
    Orderbook,
    Candle,
    Funding,
    Unknown(i32),
}

impl StreamDataType {
    pub fn from_proto_i32(v: i32) -> Self {
        match DataType::try_from(v).unwrap_or(DataType::Unknown) {
            DataType::Trade => StreamDataType::Trade,
            DataType::Orderbook => StreamDataType::Orderbook,
            DataType::Candle => StreamDataType::Candle,
            DataType::Funding => StreamDataType::Funding,
            DataType::Unknown => StreamDataType::Unknown(v),
        }
    }

    pub fn to_proto_i32(self) -> i32 {
        match self {
            StreamDataType::Trade => DataType::Trade as i32,
            StreamDataType::Orderbook => DataType::Orderbook as i32,
            StreamDataType::Candle => DataType::Candle as i32,
            StreamDataType::Funding => DataType::Funding as i32,
            StreamDataType::Unknown(v) => v,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            StreamDataType::Trade => ":TRADE",
            StreamDataType::Orderbook => ":ORDERBOOK",
            StreamDataType::Candle => ":CANDLE",
            StreamDataType::Funding => ":FUNDING",
            StreamDataType::Unknown(_) => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StreamKey {
    pub symbol: String,
    pub venue: Option<String>,
    pub data_type: StreamDataType,
}

impl StreamKey {
    pub fn from_control_with_datatype(symbol: &str, exchange: &str, data_type: i32) -> Self {
        let symbol = symbol.trim().to_uppercase();
        let venue = match exchange.trim() {
            "" => None,
            v => Some(v.to_uppercase()),
        };

        Self {
            symbol,
            venue,
            data_type: StreamDataType::from_proto_i32(data_type),
        }
    }

    pub fn from_market_request(symbol: &str, venue: &str, data_type: i32) -> Self {
        let venue = match venue.trim() {
            "" => None,
            v => Some(v.to_uppercase()),
        };
        Self {
            symbol: symbol.trim().to_uppercase(),
            venue,
            data_type: StreamDataType::from_proto_i32(data_type),
        }
    }

    /// Symbol string as historically used at the control boundary (datatype was encoded as a suffix).
    /// Kept for display / compatibility, but control requests should provide `data_type` explicitly.
    pub fn control_symbol(&self) -> String {
        format!("{}{}", self.symbol, self.data_type.suffix())
    }
}

impl fmt::Display for StreamKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Keep the output parseable and unambiguous:
        // - Control symbol uses ':' suffixes (e.g. BTCUSDT:ORDERBOOK)
        // - Venue is separated by '#'
        // Example: BTCUSDT:ORDERBOOK#BINANCE_SPOT
        match &self.venue {
            Some(v) => write!(f, "{}#{}", self.control_symbol(), v),
            None => write!(f, "{}", self.control_symbol()),
        }
    }
}

// NOTE: We intentionally do not parse datatype from symbol suffixes anymore.
// Control requests now carry `data_type` explicitly.



use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    BinanceSpot,
    BinanceFutures,
    // Coinbase,
    // Kraken,
    // Bybit,
    // OKX,
    // Deribit,
}

impl fmt::Display for Exchange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Exchange::BinanceSpot => write!(f, "binance_spot"),
            Exchange::BinanceFutures => write!(f, "binance_futures"),
            // Exchange::Coinbase => write!(f, "coinbase"),
            // Exchange::Kraken => write!(f, "kraken"),
            // Exchange::Bybit => write!(f, "bybit"),
            // Exchange::OKX => write!(f, "okx"),
            // Exchange::Deribit => write!(f, "deribit"),
        }
    }
}

impl Exchange {
    pub fn from_identifier(identifier: &str) -> Option<Self> {
        match identifier {
            "binance_spot" => Some(Exchange::BinanceSpot),
            "binance_futures" => Some(Exchange::BinanceFutures),
            // "coinbase" => Some(Exchange::Coinbase),
            // "kraken" => Some(Exchange::Kraken),
            // "bybit" => Some(Exchange::Bybit),
            // "okx" => Some(Exchange::OKX),
            // "deribit" => Some(Exchange::Deribit),
            _ => None,
        }
    }

    pub fn websocket_url(&self) -> &'static str {
        match self {
            Exchange::BinanceSpot => "wss://stream.binance.com:9443/ws",
            Exchange::BinanceFutures => "wss://fstream.binance.com/stream",
            // Exchange::Coinbase => "wss://ws-feed.exchange.coinbase.com",
            // Exchange::Kraken => "wss://ws.kraken.com",
            // Exchange::Bybit => "wss://stream.bybit.com/v5/public/spot",
            // Exchange::OKX => "wss://ws.okx.com:8443/ws/v5/public",
            // Exchange::Deribit => "wss://www.deribit.com/ws/api/v2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataMessage {
    pub exchange: Exchange,
    pub symbol: String,
    pub timestamp: i64,
    pub data: MarketData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketData {
    Ticker {
        price: f64,
        weighted_average_price: f64,
    },
    OrderBook {
        bids: Vec<(f64, f64)>,
        asks: Vec<(f64, f64)>,
    },
    SpotTrade {
        price: f64,
        size: f64,
        side: TradeSide,
        trade_id: String,
    },
    FutureTrade {
        price: f64,
        size: f64,
        side: TradeSide,
        trade_id: String,
        contract_type: String,
        funding_ratio: Option<f64>,
    },
    Candle {
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        timestamp: i64,
        interval: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRequest {
    pub exchange: Exchange,
    pub symbol: String,
    pub data_type: DataType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    Ticker,
    OrderBook,
    SpotTrade,
    FutureTrade,
    Candle,
}

/// Parse exchange and symbol from a composite storage key (format: "exchange:symbol")
pub fn parse_exchange_symbol_key(key: &str) -> Option<(Exchange, String)> {
    let (exchange_id, symbol) = key.split_once(':')?;
    Exchange::from_identifier(exchange_id).map(|exchange| (exchange, symbol.to_string()))
}

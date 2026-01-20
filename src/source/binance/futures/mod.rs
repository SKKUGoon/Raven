pub mod klines;
pub(crate) mod parsing;
mod worker;

pub(crate) use parsing::candle::parse_binance_futures_candle;
pub use worker::{new, BinanceFuturesService};

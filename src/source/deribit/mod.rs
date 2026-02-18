//! Deribit BTC options source: ticker, trades, price index — each as a separate microservice.

pub mod client;
pub(crate) mod constants;
pub mod parsing;
mod service;

pub use client::{CHANNEL_PRICE_INDEX, CHANNEL_TICKER, CHANNEL_TRADES};
pub use service::{
    new_index_service, new_ticker_service, new_trades_service, DeribitService,
};

use crate::types::{CandleData, FundingRateData};
use dashmap::DashMap;

/// Low-frequency data storage for buffering and caching
#[derive(Debug)]
pub struct LowFrequencyStorage {
    /// Recent candles by symbol and interval
    pub candles: DashMap<String, Vec<CandleData>>,
    /// Recent funding rates by symbol
    pub funding_rates: DashMap<String, Vec<FundingRateData>>,
    /// Maximum items to keep per symbol
    pub max_items_per_symbol: usize,
}

impl LowFrequencyStorage {
    pub fn new(max_items_per_symbol: usize) -> Self {
        Self {
            candles: DashMap::new(),
            funding_rates: DashMap::new(),
            max_items_per_symbol,
        }
    }

    /// Add candle data and maintain size limit
    pub fn add_candle(&self, data: &CandleData) {
        let key = format!("{}:{}", data.symbol, data.interval);
        let mut entry = self.candles.entry(key).or_default();

        entry.push(data.clone());

        // Maintain size limit by removing oldest entries
        if entry.len() > self.max_items_per_symbol {
            let excess = entry.len() - self.max_items_per_symbol;
            entry.drain(0..excess);
        }
    }

    /// Add funding rate data and maintain size limit
    pub fn add_funding_rate(&self, data: &FundingRateData) {
        let mut entry = self.funding_rates.entry(data.symbol.clone()).or_default();

        entry.push(data.clone());

        // Maintain size limit by removing oldest entries
        if entry.len() > self.max_items_per_symbol {
            let excess = entry.len() - self.max_items_per_symbol;
            entry.drain(0..excess);
        }
    }

    /// Get recent candles for a symbol and interval
    pub fn get_candles(&self, symbol: &str, interval: &str) -> Option<Vec<CandleData>> {
        let key = format!("{symbol}:{interval}");
        self.candles.get(&key).map(|entry| entry.clone())
    }

    /// Get recent funding rates for a symbol
    pub fn get_funding_rates(&self, symbol: &str) -> Option<Vec<FundingRateData>> {
        self.funding_rates.get(symbol).map(|entry| entry.clone())
    }

    /// Get all active symbols with candle data
    pub fn get_candle_symbols(&self) -> Vec<String> {
        self.candles
            .iter()
            .map(|entry| {
                let key = entry.key();
                // Extract symbol from "symbol:interval" format
                key.split(':').next().unwrap_or(key).to_string()
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Get all active symbols with funding rate data
    pub fn get_funding_rate_symbols(&self) -> Vec<String> {
        self.funding_rates
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all active symbols across all data types
    pub fn get_all_symbols(&self) -> Vec<String> {
        let mut symbols = std::collections::HashSet::new();
        symbols.extend(self.get_candle_symbols());
        symbols.extend(self.get_funding_rate_symbols());
        symbols.into_iter().collect()
    }
}

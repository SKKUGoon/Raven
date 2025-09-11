use crate::types::{CandleData, FundingRateData};
use serde::{Deserialize, Serialize};

/// Low-frequency data types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LowFrequencyData {
    Candle(CandleData),
    FundingRate(FundingRateData),
}

/// Channel message for async processing
#[derive(Debug, Clone)]
pub struct ChannelMessage {
    pub data: LowFrequencyData,
    pub timestamp: i64,
    pub retry_count: u32,
}

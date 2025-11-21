use influxdb2::models::DataPoint;

use crate::data_engine::storage::{CandleData, FundingRateData, OrderBookSnapshot, TradeSnapshot};

use super::Result;

/// Create a DataPoint for orderbook snapshot data
pub fn create_orderbook_datapoint(snapshot: &OrderBookSnapshot) -> Result<DataPoint> {
    let mut builder = DataPoint::builder("orderbook")
        .tag("symbol", &snapshot.symbol)
        .tag("exchange", snapshot.exchange.to_string())
        .field("sequence", snapshot.sequence as i64);

    if let (Some(bid), Some(ask)) = (snapshot.bid_levels.first(), snapshot.ask_levels.first()) {
        builder = builder.field("spread", ask.price - bid.price);
    }

    for (idx, level) in snapshot.bid_levels.iter().enumerate() {
        let field_price = format!("bl{:02}_price", idx + 1);
        let field_qty = format!("bl{:02}_qty", idx + 1);
        builder = builder.field(field_price, level.price);
        builder = builder.field(field_qty, level.quantity);
    }

    for (idx, level) in snapshot.ask_levels.iter().enumerate() {
        let field_price = format!("al{:02}_price", idx + 1);
        let field_qty = format!("al{:02}_qty", idx + 1);
        builder = builder.field(field_price, level.price);
        builder = builder.field(field_qty, level.quantity);
    }

    builder
        .timestamp(snapshot.timestamp * 1_000_000)
        .build()
        .map_err(|e| {
            crate::raven_error!(
                data_serialization,
                format!("Failed to create orderbook DataPoint: {e}")
            )
        })
}

/// Create a DataPoint for trade snapshot data
pub fn create_trade_datapoint(snapshot: &TradeSnapshot) -> Result<DataPoint> {
    let side_str = snapshot.side.as_str();
    DataPoint::builder("trades")
        .tag("symbol", &snapshot.symbol)
        .tag("side", side_str)
        .tag("exchange", snapshot.exchange.to_string())
        .field("price", snapshot.price)
        .field("quantity", snapshot.quantity)
        .field("trade_id", snapshot.trade_id as i64)
        .field("trade_value", snapshot.price * snapshot.quantity)
        .timestamp(snapshot.timestamp * 1_000_000)
        .build()
        .map_err(|e| {
            crate::raven_error!(
                data_serialization,
                format!("Failed to create trade DataPoint: {e}")
            )
        })
}

/// Create a DataPoint for candle data
pub fn create_candle_datapoint(candle: &CandleData) -> Result<DataPoint> {
    DataPoint::builder("candles")
        .tag("symbol", &candle.symbol)
        .tag("interval", &candle.interval)
        .tag("exchange", candle.exchange.to_string())
        .field("open", candle.open)
        .field("high", candle.high)
        .field("low", candle.low)
        .field("close", candle.close)
        .field("volume", candle.volume)
        .timestamp(candle.timestamp)
        .build()
        .map_err(|e| {
            crate::raven_error!(
                data_serialization,
                format!("Failed to create candle DataPoint: {e}")
            )
        })
}

/// Create a DataPoint for funding rate data
pub fn create_funding_rate_datapoint(funding: &FundingRateData) -> Result<DataPoint> {
    DataPoint::builder("funding_rates")
        .tag("symbol", &funding.symbol)
        .tag("exchange", funding.exchange.to_string())
        .field("rate", funding.rate)
        .field("next_funding_time", funding.next_funding_time)
        .timestamp(funding.timestamp)
        .build()
        .map_err(|e| {
            crate::raven_error!(
                data_serialization,
                format!("Failed to create funding rate DataPoint: {e}")
            )
        })
}

/// Create a DataPoint for wallet update data
pub fn create_wallet_update_datapoint(
    user_id: &str,
    asset: &str,
    available: f64,
    locked: f64,
    timestamp: i64,
) -> Result<DataPoint> {
    DataPoint::builder("wallet_updates")
        .tag("user_id", user_id)
        .tag("asset", asset)
        .field("available", available)
        .field("locked", locked)
        .field("total", available + locked)
        .timestamp(timestamp)
        .build()
        .map_err(|e| {
            crate::raven_error!(
                data_serialization,
                format!("Failed to create wallet update DataPoint: {e}")
            )
        })
}

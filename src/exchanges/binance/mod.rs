use crate::error::{RavenError, RavenResult};
use crate::exchanges::types::*;
use crate::exchanges::websocket::WebSocketParser;
use async_trait::async_trait;
use serde_json::Value;

pub mod app;

#[cfg(test)]
mod tests;

pub struct BinanceSpotParser;
pub struct BinanceFuturesParser;

impl BinanceSpotParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BinanceSpotParser {
    fn default() -> Self {
        Self::new()
    }
}

impl BinanceFuturesParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BinanceFuturesParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WebSocketParser for BinanceSpotParser {
    fn exchange(&self) -> Exchange {
        Exchange::BinanceSpot
    }

    fn create_subscription_message(
        &self,
        subscription: &SubscriptionRequest,
    ) -> RavenResult<String> {
        let stream = match subscription.data_type {
            DataType::Ticker => format!("{}@ticker", subscription.symbol.to_lowercase()),
            DataType::OrderBook => format!("{}@depth20@100ms", subscription.symbol.to_lowercase()),
            DataType::SpotTrade => format!("{}@trade", subscription.symbol.to_lowercase()),
            DataType::FutureTrade => format!("{}@aggTrade", subscription.symbol.to_lowercase()),
            DataType::Candle => format!("{}@kline_1m", subscription.symbol.to_lowercase()),
        };

        Ok(serde_json::json!({
            "method": "SUBSCRIBE",
            "params": [stream],
            "id": 1
        })
        .to_string())
    }

    fn parse_ticker(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.ends_with("@ticker") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;

                let symbol = data
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;
                let price = data
                    .get("c")
                    .and_then(|p| p.as_str())
                    .and_then(|p| p.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid price".to_string())
                    })?;
                let wap = data
                    .get("w")
                    .and_then(|c| c.as_str())
                    .and_then(|c| c.parse::<f64>().ok())
                    .unwrap_or(0.0);

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::Ticker {
                        price,
                        weighted_average_price: wap,
                    },
                }));
            }
        }

        Ok(None)
    }

    fn parse_orderbook(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.contains("@depth") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;

                let symbol = data
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;

                let bids: Vec<(f64, f64)> = data
                    .get("bids")
                    .and_then(|b| b.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|bid| {
                                if let Some(bid_arr) = bid.as_array() {
                                    if bid_arr.len() >= 2 {
                                        let price = bid_arr[0].as_str()?.parse::<f64>().ok()?;
                                        let qty = bid_arr[1].as_str()?.parse::<f64>().ok()?;
                                        Some((price, qty))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let asks: Vec<(f64, f64)> = data
                    .get("asks")
                    .and_then(|a| a.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|ask| {
                                if let Some(ask_arr) = ask.as_array() {
                                    if ask_arr.len() >= 2 {
                                        let price = ask_arr[0].as_str()?.parse::<f64>().ok()?;
                                        let qty = ask_arr[1].as_str()?.parse::<f64>().ok()?;
                                        Some((price, qty))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::OrderBook { bids, asks },
                }));
            }
        }

        Ok(None)
    }

    fn parse_spot_trade(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.ends_with("@trade") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;

                let symbol = data
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;
                let price = data
                    .get("p")
                    .and_then(|p| p.as_str())
                    .and_then(|p| p.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid price".to_string())
                    })?;
                let size = data
                    .get("q")
                    .and_then(|q| q.as_str())
                    .and_then(|q| q.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid quantity".to_string())
                    })?;
                let side = if data.get("m").and_then(|m| m.as_bool()).unwrap_or(false) {
                    TradeSide::Sell
                } else {
                    TradeSide::Buy
                };
                let trade_id = data
                    .get("t")
                    .and_then(|t| t.as_u64())
                    .map(|t| t.to_string())
                    .ok_or_else(|| RavenError::parse_error("Missing trade ID".to_string()))?;

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::SpotTrade {
                        price,
                        size,
                        side,
                        trade_id,
                    },
                }));
            }
        }

        Ok(None)
    }

    fn parse_future_trade(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.ends_with("@aggTrade") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;

                let symbol = data
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;
                let price = data
                    .get("p")
                    .and_then(|p| p.as_str())
                    .and_then(|p| p.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid price".to_string())
                    })?;
                let size = data
                    .get("q")
                    .and_then(|q| q.as_str())
                    .and_then(|q| q.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid quantity".to_string())
                    })?;
                let side = if data.get("m").and_then(|m| m.as_bool()).unwrap_or(false) {
                    TradeSide::Sell
                } else {
                    TradeSide::Buy
                };
                let trade_id = data
                    .get("a")
                    .and_then(|a| a.as_u64())
                    .map(|a| a.to_string())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing aggregate trade ID".to_string())
                    })?;

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::FutureTrade {
                        price,
                        size,
                        side,
                        trade_id,
                        contract_type: "PERPETUAL".to_string(),
                        funding_ratio: None,
                    },
                }));
            }
        }

        Ok(None)
    }

    fn parse_candle(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.contains("@kline") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;
                let kline = data
                    .get("k")
                    .ok_or_else(|| RavenError::parse_error("Missing kline data".to_string()))?;

                let symbol = kline
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;
                let open = kline
                    .get("o")
                    .and_then(|o| o.as_str())
                    .and_then(|o| o.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid open price".to_string())
                    })?;
                let high = kline
                    .get("h")
                    .and_then(|h| h.as_str())
                    .and_then(|h| h.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid high price".to_string())
                    })?;
                let low = kline
                    .get("l")
                    .and_then(|l| l.as_str())
                    .and_then(|l| l.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid low price".to_string())
                    })?;
                let close = kline
                    .get("c")
                    .and_then(|c| c.as_str())
                    .and_then(|c| c.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid close price".to_string())
                    })?;
                let volume = kline
                    .get("v")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid volume".to_string())
                    })?;
                let timestamp = kline.get("t").and_then(|t| t.as_i64()).ok_or_else(|| {
                    RavenError::parse_error("Missing or invalid timestamp".to_string())
                })?;
                let interval = kline
                    .get("i")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing interval".to_string()))?;

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::Candle {
                        open,
                        high,
                        low,
                        close,
                        volume,
                        timestamp,
                        interval: interval.to_string(),
                    },
                }));
            }
        }

        Ok(None)
    }
}

#[async_trait]
impl WebSocketParser for BinanceFuturesParser {
    fn exchange(&self) -> Exchange {
        Exchange::BinanceFutures
    }

    fn create_subscription_message(
        &self,
        subscription: &SubscriptionRequest,
    ) -> RavenResult<String> {
        let stream = match subscription.data_type {
            DataType::Ticker => format!("{}@ticker", subscription.symbol.to_lowercase()),
            DataType::OrderBook => format!("{}@depth20@100ms", subscription.symbol.to_lowercase()),
            DataType::SpotTrade => format!("{}@trade", subscription.symbol.to_lowercase()), // Not applicable for futures
            DataType::FutureTrade => format!("{}@aggTrade", subscription.symbol.to_lowercase()),
            DataType::Candle => format!("{}@kline_1m", subscription.symbol.to_lowercase()),
        };

        Ok(serde_json::json!({
            "method": "SUBSCRIBE",
            "params": [stream],
            "id": 1
        })
        .to_string())
    }

    fn parse_ticker(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.ends_with("@ticker") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;

                let symbol = data
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;
                let price = data
                    .get("c")
                    .and_then(|p| p.as_str())
                    .and_then(|p| p.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid price".to_string())
                    })?;
                let wap = data
                    .get("w")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::Ticker {
                        price,
                        weighted_average_price: wap,
                    },
                }));
            }
        }

        Ok(None)
    }

    fn parse_orderbook(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        // Same implementation as spot for orderbook
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.contains("@depth") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;

                let symbol = data
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;

                let bids: Vec<(f64, f64)> = data
                    .get("bids")
                    .and_then(|b| b.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|bid| {
                                if let Some(bid_arr) = bid.as_array() {
                                    if bid_arr.len() >= 2 {
                                        let price = bid_arr[0].as_str()?.parse::<f64>().ok()?;
                                        let qty = bid_arr[1].as_str()?.parse::<f64>().ok()?;
                                        Some((price, qty))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let asks: Vec<(f64, f64)> = data
                    .get("asks")
                    .and_then(|a| a.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|ask| {
                                if let Some(ask_arr) = ask.as_array() {
                                    if ask_arr.len() >= 2 {
                                        let price = ask_arr[0].as_str()?.parse::<f64>().ok()?;
                                        let qty = ask_arr[1].as_str()?.parse::<f64>().ok()?;
                                        Some((price, qty))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::OrderBook { bids, asks },
                }));
            }
        }

        Ok(None)
    }

    fn parse_spot_trade(&self, _message: &str) -> RavenResult<Option<MarketDataMessage>> {
        // Not applicable for futures, return None
        Ok(None)
    }

    fn parse_future_trade(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.ends_with("@aggTrade") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;

                let symbol = data
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;
                let price = data
                    .get("p")
                    .and_then(|p| p.as_str())
                    .and_then(|p| p.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid price".to_string())
                    })?;
                let size = data
                    .get("q")
                    .and_then(|q| q.as_str())
                    .and_then(|q| q.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid quantity".to_string())
                    })?;
                let side = if data.get("m").and_then(|m| m.as_bool()).unwrap_or(false) {
                    TradeSide::Sell
                } else {
                    TradeSide::Buy
                };
                let trade_id = data
                    .get("a")
                    .and_then(|a| a.as_u64())
                    .map(|a| a.to_string())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing aggregate trade ID".to_string())
                    })?;

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::FutureTrade {
                        price,
                        size,
                        side,
                        trade_id,
                        contract_type: "PERPETUAL".to_string(),
                        funding_ratio: None, // Would need separate funding rate stream
                    },
                }));
            }
        }

        Ok(None)
    }

    fn parse_candle(&self, message: &str) -> RavenResult<Option<MarketDataMessage>> {
        // Same implementation as spot for candles
        let value: Value = serde_json::from_str(message)
            .map_err(|e| RavenError::parse_error(format!("Failed to parse JSON: {e}")))?;

        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if stream.contains("@kline") {
                let data = value
                    .get("data")
                    .ok_or_else(|| RavenError::parse_error("Missing data field".to_string()))?;
                let kline = data
                    .get("k")
                    .ok_or_else(|| RavenError::parse_error("Missing kline data".to_string()))?;

                let symbol = kline
                    .get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing symbol".to_string()))?;
                let open = kline
                    .get("o")
                    .and_then(|o| o.as_str())
                    .and_then(|o| o.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid open price".to_string())
                    })?;
                let high = kline
                    .get("h")
                    .and_then(|h| h.as_str())
                    .and_then(|h| h.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid high price".to_string())
                    })?;
                let low = kline
                    .get("l")
                    .and_then(|l| l.as_str())
                    .and_then(|l| l.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid low price".to_string())
                    })?;
                let close = kline
                    .get("c")
                    .and_then(|c| c.as_str())
                    .and_then(|c| c.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid close price".to_string())
                    })?;
                let volume = kline
                    .get("v")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<f64>().ok())
                    .ok_or_else(|| {
                        RavenError::parse_error("Missing or invalid volume".to_string())
                    })?;
                let timestamp = kline.get("t").and_then(|t| t.as_i64()).ok_or_else(|| {
                    RavenError::parse_error("Missing or invalid timestamp".to_string())
                })?;
                let interval = kline
                    .get("i")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| RavenError::parse_error("Missing interval".to_string()))?;

                return Ok(Some(MarketDataMessage {
                    exchange: self.exchange(),
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    data: MarketData::Candle {
                        open,
                        high,
                        low,
                        close,
                        volume,
                        timestamp,
                        interval: interval.to_string(),
                    },
                }));
            }
        }

        Ok(None)
    }
}

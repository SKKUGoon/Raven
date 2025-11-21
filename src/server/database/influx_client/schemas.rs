use std::collections::HashMap;

pub fn create_orderbook_measurement_schema() -> HashMap<String, String> {
    let mut schema = HashMap::new();
    schema.insert("measurement".to_string(), "orderbook".to_string());
    schema.insert("tags".to_string(), "symbol,exchange".to_string());
    schema.insert(
        "fields".to_string(),
        "best_bid_price,best_bid_quantity,best_ask_price,best_ask_quantity,sequence,spread"
            .to_string(),
    );
    schema
}

pub fn create_trades_measurement_schema() -> HashMap<String, String> {
    let mut schema = HashMap::new();
    schema.insert("measurement".to_string(), "trades".to_string());
    schema.insert("tags".to_string(), "symbol,side,exchange".to_string());
    schema.insert(
        "fields".to_string(),
        "price,quantity,trade_id,trade_value".to_string(),
    );
    schema
}

pub fn create_candles_measurement_schema() -> HashMap<String, String> {
    let mut schema = HashMap::new();
    schema.insert("measurement".to_string(), "candles".to_string());
    schema.insert("tags".to_string(), "symbol,interval,exchange".to_string());
    schema.insert(
        "fields".to_string(),
        "open,high,low,close,volume".to_string(),
    );
    schema
}

pub fn create_funding_rates_measurement_schema() -> HashMap<String, String> {
    let mut schema = HashMap::new();
    schema.insert("measurement".to_string(), "funding_rates".to_string());
    schema.insert("tags".to_string(), "symbol,exchange".to_string());
    schema.insert("fields".to_string(), "rate,next_funding_time".to_string());
    schema
}

pub fn create_wallet_updates_measurement_schema() -> HashMap<String, String> {
    let mut schema = HashMap::new();
    schema.insert("measurement".to_string(), "wallet_updates".to_string());
    schema.insert("tags".to_string(), "user_id,asset".to_string());
    schema.insert("fields".to_string(), "available,locked,total".to_string());
    schema
}

pub fn get_all_measurement_schemas() -> Vec<HashMap<String, String>> {
    vec![
        create_orderbook_measurement_schema(),
        create_trades_measurement_schema(),
        create_candles_measurement_schema(),
        create_funding_rates_measurement_schema(),
        create_wallet_updates_measurement_schema(),
    ]
}

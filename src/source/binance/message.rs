use crate::proto::market_data_message::Data;
use crate::proto::MarketDataMessage;

#[inline]
pub(crate) fn new_market_data_message(venue: &str, producer: &str, data: Data) -> MarketDataMessage {
    MarketDataMessage {
        venue: venue.to_string(),
        producer: producer.to_string(),
        data: Some(data),
    }
}

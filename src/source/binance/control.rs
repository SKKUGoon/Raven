use crate::proto::CollectionInfo;
use crate::service::{StreamDataType, StreamKey};
use dashmap::DashSet;

#[inline]
pub(crate) fn stream_key(symbol: &str, venue: &str, data_type: StreamDataType) -> StreamKey {
    StreamKey {
        symbol: symbol.trim().to_uppercase(),
        venue: Some(venue.to_string()),
        data_type,
    }
}

#[inline]
pub(crate) fn list_active_collections(active: &DashSet<StreamKey>) -> Vec<CollectionInfo> {
    active
        .iter()
        .map(|k| CollectionInfo {
            symbol: k.key().to_string(),
            status: "active".to_string(),
            subscriber_count: 0,
        })
        .collect()
}

use crate::proto::MarketDataMessage;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tonic::Status;

pub(crate) fn filtered_broadcast_stream<F>(
    tx: &broadcast::Sender<Result<MarketDataMessage, Status>>,
    symbol: String,
    predicate: F,
) -> std::pin::Pin<Box<dyn Stream<Item = Result<MarketDataMessage, Status>> + Send>>
where
    F: Fn(&MarketDataMessage, &str, bool) -> bool + Send + Sync + 'static,
{
    let wildcard = symbol.is_empty() || symbol == "*";
    let rx = tx.subscribe();
    let stream = tokio_stream::StreamExt::filter_map(BroadcastStream::new(rx), move |item| {
        let sym = symbol.clone();
        match item {
            Ok(Ok(m)) => {
                if predicate(&m, &sym, wildcard) {
                    Some(Ok(m))
                } else {
                    None
                }
            }
            Ok(Err(e)) => Some(Err(e)),
            Err(_) => Some(Err(Status::internal("Stream lagged"))),
        }
    });
    Box::pin(stream)
}

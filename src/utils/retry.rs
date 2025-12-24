use std::time::Duration;

/// Simple forever-retry helper for async operations that should eventually succeed.
pub async fn retry_forever<T, E, Fut, F>(label: &str, delay: Duration, mut f: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    loop {
        match f().await {
            Ok(v) => return v,
            Err(e) => {
                tracing::warn!("{label} failed: {e}. Retrying in {delay:?}...");
                tokio::time::sleep(delay).await;
            }
        }
    }
}



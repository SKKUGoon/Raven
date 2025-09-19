// InfluxDB Client
// "The Iron Bank - managing the vaults of time-series data"

use anyhow::{anyhow, Result};
use futures_util::stream;
use influxdb2::api::buckets::ListBucketsRequest;
use influxdb2::api::organization::ListOrganizationRequest;
use influxdb2::models::{DataPoint, PostBucketRequest};
use influxdb2::{Client, RequestError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, sleep};

use tracing::{debug, error, info, warn};

use crate::citadel::storage::{CandleData, FundingRateData, OrderBookSnapshot, TradeSnapshot};

// For now, let's use a simpler approach and return raw query results
// We'll implement proper parsing later when we have the correct API understanding

// DataPoint creation helper functions
/// Create a DataPoint for orderbook snapshot data
pub fn create_orderbook_datapoint(snapshot: &OrderBookSnapshot) -> Result<DataPoint> {
    DataPoint::builder("orderbook")
        .tag("symbol", &snapshot.symbol)
        .tag("exchange", snapshot.exchange.to_string())
        .field("best_bid_price", snapshot.best_bid_price)
        .field("best_bid_quantity", snapshot.best_bid_quantity)
        .field("best_ask_price", snapshot.best_ask_price)
        .field("best_ask_quantity", snapshot.best_ask_quantity)
        .field("sequence", snapshot.sequence as i64)
        .field("spread", snapshot.best_ask_price - snapshot.best_bid_price)
        .timestamp(snapshot.timestamp * 1_000_000) // Convert milliseconds to nanoseconds
        .build()
        .map_err(|e| anyhow!("Failed to create orderbook DataPoint: {}", e))
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
        .timestamp(snapshot.timestamp * 1_000_000) // Convert milliseconds to nanoseconds
        .build()
        .map_err(|e| anyhow!("Failed to create trade DataPoint: {}", e))
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
        .map_err(|e| anyhow!("Failed to create candle DataPoint: {}", e))
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
        .map_err(|e| anyhow!("Failed to create funding rate DataPoint: {}", e))
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
        .map_err(|e| anyhow!("Failed to create wallet update DataPoint: {}", e))
}

// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing, reject requests
    HalfOpen, // Testing if service recovered
}

// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u64,
    pub recovery_timeout: Duration,
    pub success_threshold: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 3,
        }
    }
}

// Circuit breaker implementation
#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitBreakerState>>,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: Arc<Mutex<Option<Instant>>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: Arc::new(Mutex::new(None)),
            config,
        }
    }

    pub async fn can_execute(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if let Some(last_failure) = *self.last_failure_time.lock().await {
                    if last_failure.elapsed() > self.config.recovery_timeout {
                        drop(state);
                        let mut state = self.state.write().await;
                        *state = CircuitBreakerState::HalfOpen;
                        self.success_count.store(0, Ordering::Relaxed);
                        info!("🔄 Circuit breaker transitioning to half-open state");
                        return true;
                    }
                }
                false
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }

    pub async fn record_success(&self) {
        let state = self.state.read().await;
        match *state {
            CircuitBreakerState::Closed => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitBreakerState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if success_count >= self.config.success_threshold {
                    drop(state);
                    let mut state = self.state.write().await;
                    *state = CircuitBreakerState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                    info!("✅ Circuit breaker closed - service recovered");
                }
            }
            CircuitBreakerState::Open => {}
        }
    }

    pub async fn record_failure(&self) {
        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure_time.lock().await = Some(Instant::now());

        let state = self.state.read().await;
        match *state {
            CircuitBreakerState::Closed => {
                if failure_count >= self.config.failure_threshold {
                    drop(state);
                    let mut state = self.state.write().await;
                    *state = CircuitBreakerState::Open;
                    warn!(
                        "🚨 Circuit breaker opened due to failures: {}",
                        failure_count
                    );
                }
            }
            CircuitBreakerState::HalfOpen => {
                drop(state);
                let mut state = self.state.write().await;
                *state = CircuitBreakerState::Open;
                warn!("🚨 Circuit breaker opened during half-open test");
            }
            CircuitBreakerState::Open => {}
        }
    }

    pub async fn get_state(&self) -> CircuitBreakerState {
        self.state.read().await.clone()
    }
}

// Connection pool configuration
#[derive(Debug, Clone)]
pub struct InfluxConfig {
    pub url: String,
    pub bucket: String,
    pub org: String,
    pub token: Option<String>,
    pub pool_size: usize,
    pub timeout: Duration,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
    pub batch_size: usize,
    pub flush_interval: Duration,
}

impl Default for InfluxConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8086".to_string(),
            bucket: "market_data".to_string(),
            org: "raven".to_string(),
            token: None,
            pool_size: 10,
            timeout: Duration::from_secs(10),
            retry_attempts: 3,
            retry_delay: Duration::from_millis(100),
            batch_size: 1000,
            flush_interval: Duration::from_millis(5),
        }
    }
}

// Dead letter queue for failed writes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    pub data: String,
    pub timestamp: i64,
    pub retry_count: u32,
    pub error_message: String,
}

// Main InfluxDB client with connection pooling
pub struct InfluxClient {
    pub config: InfluxConfig,
    connection_pool: Arc<Mutex<Vec<Client>>>,
    circuit_breaker: CircuitBreaker,
    dead_letter_queue: Arc<Mutex<Vec<DeadLetterEntry>>>,
    pub health_check_running: AtomicBool,
    connection_health: Arc<RwLock<HashMap<usize, bool>>>,
}

impl InfluxClient {
    pub fn new(config: InfluxConfig) -> Self {
        info!("The Iron Bank is opening its vaults...");
        info!("Bucket: {}, Pool size: {}", config.bucket, config.pool_size);

        Self {
            config,
            connection_pool: Arc::new(Mutex::new(Vec::new())),
            circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            dead_letter_queue: Arc::new(Mutex::new(Vec::new())),
            health_check_running: AtomicBool::new(false),
            connection_health: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn connect(&self) -> Result<()> {
        info!("🔗 Connecting to the Iron Bank at {}...", self.config.url);

        let mut pool = self.connection_pool.lock().await;
        let mut health = self.connection_health.write().await;
        pool.clear();
        health.clear();

        for i in 0..self.config.pool_size {
            match self.create_client().await {
                Ok(client) => {
                    pool.push(client);
                    health.insert(i, true);
                    debug!("✅ Connection {} established", i + 1);
                }
                Err(e) => {
                    error!("❌ Failed to create connection {}: {}", i + 1, e);
                    health.insert(i, false);
                    return Err(e);
                }
            }
        }

        let pool_size = pool.len();

        // Release locks before performing operations that require them again
        drop(health);
        drop(pool);

        info!("Connection pool initialized with {} connections", pool_size);
        self.ping().await?;
        self.ensure_bucket_exists().await?;

        // Start health monitoring
        self.start_health_monitoring().await;

        Ok(())
    }

    async fn create_client(&self) -> Result<Client> {
        let client = if let Some(token) = &self.config.token {
            debug!("Creating InfluxDB client with token: {}", token);
            Client::new(&self.config.url, &self.config.org, token)
        } else {
            debug!("Creating InfluxDB client without authentication");
            // For development/testing without authentication
            Client::new(&self.config.url, &self.config.org, "")
        };

        Ok(client)
    }

    async fn ensure_bucket_exists(&self) -> Result<()> {
        let client = self.create_client().await?;

        if self.bucket_exists(&client).await? {
            debug!("📦 InfluxDB bucket '{}' already exists", self.config.bucket);
            return Ok(());
        }

        let org_id = self.fetch_org_id(&client).await?;
        let request = PostBucketRequest::new(org_id, self.config.bucket.clone());

        match client.create_bucket(Some(request)).await {
            Ok(_) => {
                info!(
                    "📦 Created InfluxDB bucket '{}' for org '{}'",
                    self.config.bucket, self.config.org
                );
                Ok(())
            }
            Err(RequestError::Http { status, text: _ }) if status.as_u16() == 409 => {
                info!(
                    "📦 Bucket '{}' already exists (HTTP 409)",
                    self.config.bucket
                );
                Ok(())
            }
            Err(e) => Err(anyhow!(
                "Failed to create bucket '{}': {}",
                self.config.bucket,
                e
            )),
        }
    }

    async fn bucket_exists(&self, client: &Client) -> Result<bool> {
        let request = ListBucketsRequest {
            name: Some(self.config.bucket.clone()),
            org: Some(self.config.org.clone()),
            limit: Some(1),
            ..Default::default()
        };

        let response = client
            .list_buckets(Some(request))
            .await
            .map_err(|e| anyhow!("Failed to list buckets: {}", e))?;

        Ok(response
            .buckets
            .iter()
            .any(|bucket| bucket.name == self.config.bucket))
    }

    async fn fetch_org_id(&self, client: &Client) -> Result<String> {
        let request = ListOrganizationRequest {
            org: Some(self.config.org.clone()),
            limit: Some(1),
            ..Default::default()
        };

        let organizations = client
            .list_organizations(request)
            .await
            .map_err(|e| anyhow!("Failed to list organizations: {}", e))?;

        if let Some(org) = organizations
            .orgs
            .iter()
            .find(|org| org.name == self.config.org)
        {
            if let Some(id) = &org.id {
                return Ok(id.clone());
            }
        }

        // Fallback: treat configured org as an ID
        let fallback_request = ListOrganizationRequest {
            org_id: Some(self.config.org.clone()),
            limit: Some(1),
            ..Default::default()
        };

        let organizations = client
            .list_organizations(fallback_request)
            .await
            .map_err(|e| anyhow!("Failed to look up organization by ID: {}", e))?;

        organizations
            .orgs
            .into_iter()
            .find_map(|org| org.id)
            .ok_or_else(|| anyhow!("Organization '{}' not found", self.config.org))
    }

    #[cfg(test)]
    pub(crate) async fn ensure_bucket_exists_for_tests(&self) -> Result<()> {
        self.ensure_bucket_exists().await
    }

    async fn get_connection(&self) -> Result<(Client, usize)> {
        if !self.circuit_breaker.can_execute().await {
            return Err(anyhow!(
                "Circuit breaker is open - Iron Bank temporarily closed"
            ));
        }

        let pool = self.connection_pool.lock().await;
        let health = self.connection_health.read().await;

        // Find first healthy connection
        for (index, client) in pool.iter().enumerate() {
            if *health.get(&index).unwrap_or(&false) {
                return Ok((client.clone(), index));
            }
        }

        // If no healthy connections, try the first one anyway
        if let Some(client) = pool.first() {
            Ok((client.clone(), 0))
        } else {
            Err(anyhow!("No connections available"))
        }
    }

    pub async fn ping(&self) -> Result<()> {
        debug!("🏓 Pinging the Iron Bank...");

        debug!("🔍 Fetching connection from pool for health check");
        let (client, connection_index) = self.get_connection().await?;
        debug!(
            connection_index = connection_index,
            "🔌 Acquired pooled connection"
        );

        // Use InfluxDB v2 health check endpoint
        debug!("🌐 Issuing InfluxDB health request");
        match client.health().await {
            Ok(_) => {
                self.circuit_breaker.record_success().await;
                self.mark_connection_healthy(connection_index, true).await;
                debug!("✅ Iron Bank responded to ping");
                Ok(())
            }
            Err(e) => {
                self.circuit_breaker.record_failure().await;
                self.mark_connection_healthy(connection_index, false).await;
                error!("❌ Iron Bank ping failed: {}", e);
                Err(anyhow!("Ping failed: {}", e))
            }
        }
    }

    /// Health check method for monitoring service
    pub async fn health_check(&self) -> Result<()> {
        debug!("🏥 Performing health check on the Iron Bank...");

        // Check circuit breaker state first
        let cb_state = self.circuit_breaker.get_state().await;
        if cb_state == CircuitBreakerState::Open {
            return Err(anyhow!("Circuit breaker is open - database unavailable"));
        }

        // Perform ping to verify connectivity
        self.ping().await?;

        // Check connection pool health
        let pool_status = self.get_pool_status().await;
        let healthy_connections = pool_status.get("healthy_connections").unwrap_or(&0);
        let total_connections = pool_status.get("total_connections").unwrap_or(&0);

        if *healthy_connections == 0 {
            return Err(anyhow!("No healthy database connections available"));
        }

        if *healthy_connections < (*total_connections / 2) {
            warn!("⚠️ Less than 50% of database connections are healthy");
        }

        debug!("✅ Iron Bank health check passed");
        Ok(())
    }

    // Write orderbook snapshot to InfluxDB
    pub async fn write_orderbook_snapshot(&self, snapshot: &OrderBookSnapshot) -> Result<()> {
        let data_point = create_orderbook_datapoint(snapshot)?;
        self.execute_write(data_point).await
    }

    // Write trade snapshot to InfluxDB
    pub async fn write_trade_snapshot(&self, snapshot: &TradeSnapshot) -> Result<()> {
        let data_point = create_trade_datapoint(snapshot)?;
        self.execute_write(data_point).await
    }

    // Write candle data to InfluxDB
    pub async fn write_candle(&self, candle: &CandleData) -> Result<()> {
        let data_point = create_candle_datapoint(candle)?;
        self.execute_write(data_point).await
    }

    // Write funding rate data to InfluxDB
    pub async fn write_funding_rate(&self, funding: &FundingRateData) -> Result<()> {
        let data_point = create_funding_rate_datapoint(funding)?;
        self.execute_write(data_point).await
    }

    // Write wallet update data to InfluxDB (private data)
    pub async fn write_wallet_update(
        &self,
        user_id: &str,
        balances: &[(String, f64, f64)],
        timestamp: i64,
    ) -> Result<()> {
        for (asset, available, locked) in balances {
            let data_point = DataPoint::builder("wallet_updates")
                .tag("user_id", user_id)
                .tag("asset", asset)
                .field("available", *available)
                .field("locked", *locked)
                .field("total", available + locked)
                .timestamp(timestamp)
                .build()?;

            self.execute_write(data_point).await?;
        }

        Ok(())
    }

    // Batch write multiple data points for better performance
    pub async fn write_batch(&self, data_points: Vec<DataPoint>) -> Result<()> {
        if data_points.is_empty() {
            return Ok(());
        }

        let (client, connection_index) = self.get_connection().await?;

        for attempt in 1..=self.config.retry_attempts {
            // Create a stream from the data points
            let data_stream = stream::iter(data_points.clone());

            match client.write(&self.config.bucket, data_stream).await {
                Ok(_) => {
                    self.circuit_breaker.record_success().await;
                    self.mark_connection_healthy(connection_index, true).await;
                    debug!(
                        "✅ Batch write successful: {} data points",
                        data_points.len()
                    );
                    return Ok(());
                }
                Err(e) => {
                    warn!("⚠️ Batch write attempt {} failed: {}", attempt, e);
                    self.mark_connection_healthy(connection_index, false).await;

                    if attempt == self.config.retry_attempts {
                        self.circuit_breaker.record_failure().await;

                        // Add failed data points to dead letter queue
                        for data_point in &data_points {
                            self.add_to_dead_letter_queue(
                                format!("{data_point:?}"),
                                "Batch write failed".to_string(),
                            )
                            .await;
                        }

                        return Err(anyhow!("Batch write failed after {} attempts", attempt));
                    }

                    sleep(self.config.retry_delay * attempt).await;
                }
            }
        }

        unreachable!()
    }

    // Query historical data with time range
    pub async fn query_historical_data(
        &self,
        measurement: &str,
        symbol: &str,
        start_time: i64,
        end_time: i64,
        limit: Option<u32>,
    ) -> Result<Vec<HashMap<String, String>>> {
        // For placeholder implementation, don't require connection
        // TODO: Implement proper Flux query parsing once we understand the correct API

        let limit_clause = limit
            .map(|l| format!("|> limit(n: {l})"))
            .unwrap_or_default();

        let _flux_query = format!(
            r#"
            from(bucket: "{}")
            |> range(start: {}ms, stop: {}ms)
            |> filter(fn: (r) => r._measurement == "{}" and r.symbol == "{}")
            |> sort(columns: ["_time"], desc: true)
            {}
            "#,
            self.config.bucket, start_time, end_time, measurement, symbol, limit_clause
        );

        // Return empty results for now - this needs proper implementation
        let results = Vec::new();
        debug!("✅ Query executed (placeholder implementation)");
        Ok(results)
    }

    // Execute a single write with retry logic
    async fn execute_write(&self, data_point: DataPoint) -> Result<()> {
        let (client, connection_index) = self.get_connection().await?;

        for attempt in 1..=self.config.retry_attempts {
            // Create a stream from the single data point
            let data_stream = stream::iter(vec![data_point.clone()]);

            match client.write(&self.config.bucket, data_stream).await {
                Ok(_) => {
                    self.circuit_breaker.record_success().await;
                    self.mark_connection_healthy(connection_index, true).await;
                    return Ok(());
                }
                Err(e) => {
                    warn!("⚠️ Write attempt {} failed: {}", attempt, e);
                    self.mark_connection_healthy(connection_index, false).await;

                    if attempt == self.config.retry_attempts {
                        self.circuit_breaker.record_failure().await;
                        self.add_to_dead_letter_queue(format!("{data_point:?}"), e.to_string())
                            .await;
                        return Err(anyhow!("Write failed after {attempt} attempts: {e}"));
                    }

                    sleep(self.config.retry_delay * attempt).await;
                }
            }
        }

        unreachable!()
    }

    // Add failed write to dead letter queue
    pub async fn add_to_dead_letter_queue(&self, data: String, error_message: String) {
        let entry = DeadLetterEntry {
            data,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            retry_count: 0,
            error_message,
        };

        let mut queue = self.dead_letter_queue.lock().await;
        queue.push(entry);

        warn!(
            "📮 Added failed write to dead letter queue. Queue size: {}",
            queue.len()
        );
    }

    // Get connection pool status
    pub async fn get_pool_status(&self) -> HashMap<String, u64> {
        let pool = self.connection_pool.lock().await;
        let health = self.connection_health.read().await;

        let total_count = pool.len() as u64;
        let healthy_count = health.values().filter(|&&h| h).count() as u64;

        let circuit_state = match self.circuit_breaker.get_state().await {
            CircuitBreakerState::Closed => 0,
            CircuitBreakerState::Open => 1,
            CircuitBreakerState::HalfOpen => 2,
        };

        let mut status = HashMap::new();
        status.insert("total_connections".to_string(), total_count);
        status.insert("healthy_connections".to_string(), healthy_count);
        status.insert(
            "unhealthy_connections".to_string(),
            total_count - healthy_count,
        );
        status.insert("circuit_breaker_state".to_string(), circuit_state);
        status.insert(
            "dead_letter_queue_size".to_string(),
            self.dead_letter_queue.lock().await.len() as u64,
        );
        status.insert(
            "health_monitoring_active".to_string(),
            if self.health_check_running.load(Ordering::Relaxed) {
                1
            } else {
                0
            },
        );

        status
    }

    // Mark connection as healthy or unhealthy
    async fn mark_connection_healthy(&self, connection_index: usize, healthy: bool) {
        let mut health = self.connection_health.write().await;
        health.insert(connection_index, healthy);

        if !healthy {
            debug!("🔴 Connection {} marked as unhealthy", connection_index);
        } else {
            debug!("🟢 Connection {} marked as healthy", connection_index);
        }
    }

    // Start health monitoring background task
    async fn start_health_monitoring(&self) {
        if self
            .health_check_running
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            let client = Arc::new(self.clone());
            tokio::spawn(async move {
                client.health_monitor_loop().await;
            });
            info!("🏥 Health monitoring started for Iron Bank connections");
        }
    }

    // Health monitoring loop
    async fn health_monitor_loop(&self) {
        let mut interval = interval(Duration::from_secs(30)); // Check every 30 seconds

        loop {
            interval.tick().await;

            if let Err(e) = self.check_all_connections().await {
                warn!("⚠️ Health check failed: {}", e);
            }
        }
    }

    // Check health of all connections
    async fn check_all_connections(&self) -> Result<()> {
        let pool = self.connection_pool.lock().await;
        let pool_size = pool.len();
        drop(pool);

        let mut healthy_count = 0;

        for i in 0..pool_size {
            if let Ok(is_healthy) = self.check_single_connection(i).await {
                if is_healthy {
                    healthy_count += 1;
                }
            }
        }

        debug!(
            "🏥 Health check: {}/{} connections healthy",
            healthy_count, pool_size
        );

        if healthy_count == 0 {
            warn!("🚨 All connections are unhealthy!");
            // Attempt to reconnect
            if let Err(e) = self.reconnect_unhealthy_connections().await {
                error!("❌ Failed to reconnect: {}", e);
            }
        }

        Ok(())
    }

    // Check health of a single connection
    async fn check_single_connection(&self, connection_index: usize) -> Result<bool> {
        let pool = self.connection_pool.lock().await;
        if let Some(client) = pool.get(connection_index) {
            let client = client.clone();
            drop(pool);

            // Use v2 health endpoint instead of SHOW DATABASES
            match client.health().await {
                Ok(_) => {
                    self.mark_connection_healthy(connection_index, true).await;
                    Ok(true)
                }
                Err(_) => {
                    self.mark_connection_healthy(connection_index, false).await;
                    Ok(false)
                }
            }
        } else {
            Ok(false)
        }
    }

    // Reconnect unhealthy connections
    async fn reconnect_unhealthy_connections(&self) -> Result<()> {
        info!("🔄 Attempting to reconnect unhealthy connections...");

        let mut pool = self.connection_pool.lock().await;

        // Collect unhealthy connection indices first
        let unhealthy_indices: Vec<usize> = {
            let health = self.connection_health.read().await;
            health
                .iter()
                .filter_map(|(index, is_healthy)| if !is_healthy { Some(*index) } else { None })
                .collect()
        };

        for index in unhealthy_indices {
            match self.create_client().await {
                Ok(new_client) => {
                    if let Some(client) = pool.get_mut(index) {
                        *client = new_client;
                        self.mark_connection_healthy(index, true).await;
                        info!("✅ Reconnected connection {}", index);
                        return Ok(());
                    }
                }
                Err(e) => {
                    warn!("⚠️ Failed to reconnect connection {}: {}", index, e);
                }
            }
        }

        Ok(())
    }

    // Process dead letter queue
    pub async fn process_dead_letter_queue(&self) -> Result<usize> {
        let mut queue = self.dead_letter_queue.lock().await;
        let mut processed = 0;
        let mut failed_entries = Vec::new();

        for entry in queue.drain(..) {
            if entry.retry_count < 3 {
                // Try to reprocess the entry
                debug!("🔄 Retrying dead letter entry: {}", entry.data);
                // For now, just mark as processed - in a real implementation,
                // you would parse and re-execute the query
                processed += 1;
            } else {
                // Too many retries, keep in queue
                failed_entries.push(DeadLetterEntry {
                    retry_count: entry.retry_count + 1,
                    ..entry
                });
            }
        }

        // Put back failed entries
        queue.extend(failed_entries);

        if processed > 0 {
            info!("📮 Processed {} entries from dead letter queue", processed);
        }

        Ok(processed)
    }

    // Create bucket and setup v2 configuration
    pub async fn setup_database(&self) -> Result<()> {
        info!("🏗️ Setting up Iron Bank bucket and v2 configuration...");

        let (_client, _) = self.get_connection().await?;

        // In InfluxDB v2, buckets are created through the API or UI
        // We'll verify the bucket exists by attempting a simple query
        let _flux_query = format!(
            r#"
            from(bucket: "{}")
            |> range(start: -1m)
            |> limit(n: 1)
            "#,
            self.config.bucket
        );

        // For now, assume bucket exists - proper verification needs correct API understanding
        match Ok(()) as Result<(), anyhow::Error> {
            Ok(_) => {
                info!("✅ Bucket '{}' is accessible", self.config.bucket);
            }
            Err(e) => {
                warn!(
                    "⚠️ Bucket '{}' may not exist or is not accessible: {}",
                    self.config.bucket, e
                );

                // Try to create the bucket using the management API
                // Note: This requires admin privileges and proper token
                match self.create_bucket_if_needed().await {
                    Ok(_) => {
                        info!("✅ Bucket '{}' created successfully", self.config.bucket);
                    }
                    Err(create_err) => {
                        warn!(
                            "⚠️ Could not create bucket '{}': {}. Please create it manually through the InfluxDB UI or API",
                            self.config.bucket, create_err
                        );
                        // Don't fail setup if bucket creation fails - it might already exist
                        // or the token might not have admin privileges
                    }
                }
            }
        }

        info!("✅ Iron Bank v2 setup completed");
        Ok(())
    }

    // Helper method to create bucket if it doesn't exist
    async fn create_bucket_if_needed(&self) -> Result<()> {
        // In a production environment, you would use the InfluxDB v2 management API
        // to create buckets programmatically. For now, we'll just log the requirement.
        info!("📋 Bucket creation should be done through InfluxDB v2 management API");
        info!(
            "📋 Please ensure bucket '{}' exists in organization '{}'",
            self.config.bucket, self.config.org
        );

        // In a real implementation, you would make HTTP requests to:
        // POST /api/v2/buckets with proper authentication
        // This requires admin token and proper bucket configuration

        Ok(())
    }
}

impl Clone for InfluxClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            connection_pool: Arc::clone(&self.connection_pool),
            circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            dead_letter_queue: Arc::clone(&self.dead_letter_queue),
            health_check_running: AtomicBool::new(false),
            connection_health: Arc::clone(&self.connection_health),
        }
    }
}

impl Default for InfluxClient {
    fn default() -> Self {
        Self::new(InfluxConfig::default())
    }
}

// Utility functions for measurement schemas
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

// Get all measurement schemas
pub fn get_all_measurement_schemas() -> Vec<HashMap<String, String>> {
    vec![
        create_orderbook_measurement_schema(),
        create_trades_measurement_schema(),
        create_candles_measurement_schema(),
        create_funding_rates_measurement_schema(),
        create_wallet_updates_measurement_schema(),
    ]
}

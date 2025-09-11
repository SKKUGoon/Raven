// InfluxDB Client
// "The Iron Bank - managing the vaults of time-series data"

use anyhow::{anyhow, Context, Result};
use influxdb::{Client, ReadQuery, Timestamp, WriteQuery};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

use crate::types::{CandleData, FundingRateData, OrderBookSnapshot, TradeSnapshot};

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
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
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
            database: "market_data".to_string(),
            username: None,
            password: None,
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
    health_check_running: AtomicBool,
    connection_health: Arc<RwLock<HashMap<usize, bool>>>,
}

impl InfluxClient {
    pub fn new(config: InfluxConfig) -> Self {
        info!("🏦 The Iron Bank is opening its vaults...");
        info!(
            "📊 Database: {}, Pool size: {}",
            config.database, config.pool_size
        );

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

        info!(
            "🎯 Connection pool initialized with {} connections",
            pool.len()
        );
        self.ping().await?;

        // Start health monitoring
        self.start_health_monitoring().await;

        info!("🏦 The Iron Bank is ready for business!");
        Ok(())
    }

    async fn create_client(&self) -> Result<Client> {
        let mut client = Client::new(&self.config.url, &self.config.database);

        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            client = client.with_auth(username, password);
        }

        Ok(client)
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

        let (client, connection_index) = self.get_connection().await?;
        let query = ReadQuery::new("SHOW DATABASES");

        match client.query(query).await {
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
        let timestamp = Timestamp::Milliseconds(snapshot.timestamp as u128);

        let write_query = WriteQuery::new(timestamp, "orderbook")
            .add_tag("symbol", snapshot.symbol.as_str())
            .add_field("best_bid_price", snapshot.best_bid_price)
            .add_field("best_bid_quantity", snapshot.best_bid_quantity)
            .add_field("best_ask_price", snapshot.best_ask_price)
            .add_field("best_ask_quantity", snapshot.best_ask_quantity)
            .add_field("sequence", snapshot.sequence as i64)
            .add_field("spread", snapshot.best_ask_price - snapshot.best_bid_price);

        self.execute_write(write_query).await
    }

    // Write trade snapshot to InfluxDB
    pub async fn write_trade_snapshot(&self, snapshot: &TradeSnapshot) -> Result<()> {
        let timestamp = Timestamp::Milliseconds(snapshot.timestamp as u128);

        let write_query = WriteQuery::new(timestamp, "trades")
            .add_tag("symbol", snapshot.symbol.as_str())
            .add_tag("side", snapshot.side.as_str())
            .add_field("price", snapshot.price)
            .add_field("quantity", snapshot.quantity)
            .add_field("trade_id", snapshot.trade_id as i64)
            .add_field("trade_value", snapshot.price * snapshot.quantity);

        self.execute_write(write_query).await
    }

    // Write candle data to InfluxDB
    pub async fn write_candle(&self, candle: &CandleData) -> Result<()> {
        let timestamp = Timestamp::Milliseconds(candle.timestamp as u128);

        let write_query = WriteQuery::new(timestamp, "candles")
            .add_tag("symbol", candle.symbol.as_str())
            .add_tag("interval", candle.interval.as_str())
            .add_tag("exchange", candle.exchange.as_str())
            .add_field("open", candle.open)
            .add_field("high", candle.high)
            .add_field("low", candle.low)
            .add_field("close", candle.close)
            .add_field("volume", candle.volume);

        self.execute_write(write_query).await
    }

    // Write funding rate data to InfluxDB
    pub async fn write_funding_rate(&self, funding: &FundingRateData) -> Result<()> {
        let timestamp = Timestamp::Milliseconds(funding.timestamp as u128);

        let write_query = WriteQuery::new(timestamp, "funding_rates")
            .add_tag("symbol", funding.symbol.as_str())
            .add_tag("exchange", funding.exchange.as_str())
            .add_field("rate", funding.rate)
            .add_field("next_funding_time", funding.next_funding_time);

        self.execute_write(write_query).await
    }

    // Write wallet update data to InfluxDB (private data)
    pub async fn write_wallet_update(
        &self,
        user_id: &str,
        balances: &[(String, f64, f64)],
        timestamp: i64,
    ) -> Result<()> {
        let ts = Timestamp::Milliseconds(timestamp as u128);

        for (asset, available, locked) in balances {
            let write_query = WriteQuery::new(ts, "wallet_updates")
                .add_tag("user_id", user_id)
                .add_tag("asset", asset.as_str())
                .add_field("available", *available)
                .add_field("locked", *locked)
                .add_field("total", available + locked);

            self.execute_write(write_query).await?;
        }

        Ok(())
    }

    // Batch write multiple queries for better performance
    pub async fn write_batch(&self, queries: Vec<WriteQuery>) -> Result<()> {
        if queries.is_empty() {
            return Ok(());
        }

        let (client, connection_index) = self.get_connection().await?;

        for attempt in 1..=self.config.retry_attempts {
            let mut all_success = true;

            for query in &queries {
                match client.query(query.clone()).await {
                    Ok(_) => continue,
                    Err(e) => {
                        warn!("⚠️ Batch write attempt {} failed for query: {}", attempt, e);
                        all_success = false;
                        break;
                    }
                }
            }

            if all_success {
                self.circuit_breaker.record_success().await;
                self.mark_connection_healthy(connection_index, true).await;
                debug!("✅ Batch write successful: {} queries", queries.len());
                return Ok(());
            }

            if attempt == self.config.retry_attempts {
                self.circuit_breaker.record_failure().await;
                self.mark_connection_healthy(connection_index, false).await;

                // Add failed queries to dead letter queue
                for query in &queries {
                    self.add_to_dead_letter_queue(
                        format!("{query:?}"),
                        "Batch write failed".to_string(),
                    )
                    .await;
                }

                return Err(anyhow!("Batch write failed after {} attempts", attempt));
            }

            sleep(self.config.retry_delay * attempt).await;
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
        let (client, connection_index) = self.get_connection().await?;

        let limit_clause = limit.map(|l| format!(" LIMIT {l}")).unwrap_or_default();

        let query_str = format!(
            "SELECT * FROM {measurement} WHERE symbol = '{symbol}' AND time >= {start_time}ms AND time <= {end_time}ms ORDER BY time DESC{limit_clause}"
        );

        let query = ReadQuery::new(query_str);

        match client.query(query).await {
            Ok(_result) => {
                self.circuit_breaker.record_success().await;
                self.mark_connection_healthy(connection_index, true).await;

                // For now, return empty results - in a real implementation,
                // you would parse the InfluxDB response properly
                let results = Vec::new();

                Ok(results)
            }
            Err(e) => {
                self.circuit_breaker.record_failure().await;
                self.mark_connection_healthy(connection_index, false).await;
                Err(anyhow!("Historical query failed: {}", e))
            }
        }
    }

    // Execute a single write with retry logic
    async fn execute_write(&self, query: WriteQuery) -> Result<()> {
        let (client, connection_index) = self.get_connection().await?;

        for attempt in 1..=self.config.retry_attempts {
            match client.query(query.clone()).await {
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
                        self.add_to_dead_letter_queue(format!("{query:?}"), e.to_string())
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
    async fn add_to_dead_letter_queue(&self, data: String, error_message: String) {
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

            let query = ReadQuery::new("SHOW DATABASES");
            match client.query(query).await {
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

    // Create database and retention policies
    pub async fn setup_database(&self) -> Result<()> {
        info!("🏗️ Setting up Iron Bank database and retention policies...");

        let (client, _) = self.get_connection().await?;

        // Create database
        let create_db_query = ReadQuery::new(format!("CREATE DATABASE {}", self.config.database));
        client
            .query(create_db_query)
            .await
            .context("Failed to create database")?;

        // Create retention policies for different data types
        self.create_retention_policies(&client).await?;

        info!("✅ Iron Bank database setup completed");
        Ok(())
    }

    // Create retention policies for different measurement types
    async fn create_retention_policies(&self, client: &Client) -> Result<()> {
        let policies = vec![
            // High-frequency data: 7 days full resolution
            ("high_freq_7d", "7d", "1h", true),
            // Medium-frequency data: 30 days
            ("medium_freq_30d", "30d", "1h", false),
            // Low-frequency data: 2 years
            ("low_freq_2y", "104w", "1h", false),
        ];

        for (name, duration, shard_duration, is_default) in policies {
            let default_clause = if is_default { " DEFAULT" } else { "" };
            let policy_query = format!(
                "CREATE RETENTION POLICY {name} ON {} DURATION {duration} REPLICATION 1 SHARD DURATION {shard_duration}{default_clause}",
                self.config.database
            );

            let query = ReadQuery::new(policy_query);
            if let Err(e) = client.query(query).await {
                warn!("⚠️ Failed to create retention policy {}: {}", name, e);
            } else {
                debug!("✅ Created retention policy: {}", name);
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_influx_client_creation() {
        let config = InfluxConfig::default();
        let client = InfluxClient::new(config);

        assert_eq!(client.config.database, "market_data");
        assert_eq!(client.config.pool_size, 10);
        assert!(!client.health_check_running.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = CircuitBreakerConfig::default();
        let breaker = CircuitBreaker::new(config);

        // Initially should be closed
        assert!(breaker.can_execute().await);
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);

        // Record failures to open circuit
        for _ in 0..5 {
            breaker.record_failure().await;
        }

        assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);
        assert!(!breaker.can_execute().await);
    }

    #[test]
    fn test_measurement_schemas() {
        let orderbook_schema = create_orderbook_measurement_schema();
        assert_eq!(orderbook_schema.get("measurement").unwrap(), "orderbook");
        assert_eq!(orderbook_schema.get("tags").unwrap(), "symbol,exchange");

        let trades_schema = create_trades_measurement_schema();
        assert_eq!(trades_schema.get("measurement").unwrap(), "trades");
        assert_eq!(trades_schema.get("tags").unwrap(), "symbol,side,exchange");

        let candles_schema = create_candles_measurement_schema();
        assert_eq!(candles_schema.get("measurement").unwrap(), "candles");
        assert_eq!(
            candles_schema.get("tags").unwrap(),
            "symbol,interval,exchange"
        );

        let funding_schema = create_funding_rates_measurement_schema();
        assert_eq!(funding_schema.get("measurement").unwrap(), "funding_rates");
        assert_eq!(funding_schema.get("tags").unwrap(), "symbol,exchange");

        let wallet_schema = create_wallet_updates_measurement_schema();
        assert_eq!(wallet_schema.get("measurement").unwrap(), "wallet_updates");
        assert_eq!(wallet_schema.get("tags").unwrap(), "user_id,asset");
    }

    #[test]
    fn test_all_measurement_schemas() {
        let all_schemas = get_all_measurement_schemas();
        assert_eq!(all_schemas.len(), 5);

        let measurements: Vec<&str> = all_schemas
            .iter()
            .map(|schema| schema.get("measurement").unwrap().as_str())
            .collect();

        assert!(measurements.contains(&"orderbook"));
        assert!(measurements.contains(&"trades"));
        assert!(measurements.contains(&"candles"));
        assert!(measurements.contains(&"funding_rates"));
        assert!(measurements.contains(&"wallet_updates"));
    }

    #[tokio::test]
    async fn test_dead_letter_queue() {
        let config = InfluxConfig::default();
        let client = InfluxClient::new(config);

        // Add some entries to dead letter queue
        client
            .add_to_dead_letter_queue("test query".to_string(), "test error".to_string())
            .await;

        let status = client.get_pool_status().await;
        assert_eq!(status.get("dead_letter_queue_size").unwrap(), &1);
    }

    #[tokio::test]
    async fn test_write_operations() {
        let config = InfluxConfig::default();
        let client = InfluxClient::new(config);

        // Test orderbook snapshot write (will fail without real InfluxDB, but tests the structure)
        let snapshot = OrderBookSnapshot {
            symbol: "BTCUSDT".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            best_bid_price: 45000.0,
            best_bid_quantity: 1.5,
            best_ask_price: 45001.0,
            best_ask_quantity: 1.2,
            sequence: 12345,
        };

        // This will fail because we don't have a real InfluxDB connection,
        // but it tests that the method exists and has the right signature
        let result = client.write_orderbook_snapshot(&snapshot).await;
        assert!(result.is_err()); // Expected to fail without real connection

        // Test trade snapshot write
        let trade_snapshot = TradeSnapshot {
            symbol: "BTCUSDT".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            price: 45000.5,
            quantity: 0.1,
            side: "buy".to_string(),
            trade_id: 123456,
        };

        let result = client.write_trade_snapshot(&trade_snapshot).await;
        assert!(result.is_err()); // Expected to fail without real connection

        // Test candle write
        let candle = CandleData {
            symbol: "BTCUSDT".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            open: 45000.0,
            high: 45100.0,
            low: 44900.0,
            close: 45050.0,
            volume: 150.5,
            interval: "1m".to_string(),
            exchange: "binance".to_string(),
        };

        let result = client.write_candle(&candle).await;
        assert!(result.is_err()); // Expected to fail without real connection

        // Test funding rate write
        let funding = FundingRateData {
            symbol: "BTCUSDT".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            rate: 0.0001,
            next_funding_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
                + 28800000, // 8 hours later
            exchange: "binance".to_string(),
        };

        let result = client.write_funding_rate(&funding).await;
        assert!(result.is_err()); // Expected to fail without real connection

        // Test wallet update write
        let balances = vec![
            ("BTC".to_string(), 1.5, 0.1),
            ("USDT".to_string(), 10000.0, 500.0),
        ];

        let result = client
            .write_wallet_update(
                "user123",
                &balances,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
            )
            .await;
        assert!(result.is_err()); // Expected to fail without real connection
    }

    #[tokio::test]
    async fn test_config_defaults() {
        let config = InfluxConfig::default();

        assert_eq!(config.url, "http://localhost:8086");
        assert_eq!(config.database, "market_data");
        assert_eq!(config.pool_size, 10);
        assert_eq!(config.retry_attempts, 3);
        assert_eq!(config.batch_size, 1000);
        assert!(config.username.is_none());
        assert!(config.password.is_none());
    }
}

// Circuit Breaker Implementation
// "When the wall falls, we must know when to rebuild it"

#[cfg(test)]
mod tests;

use crate::error::RavenError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Normal operation - requests are allowed through
    Closed,
    /// Failing state - requests are rejected immediately
    Open,
    /// Testing state - limited requests are allowed to test if service recovered
    HalfOpen,
}

impl std::fmt::Display for CircuitBreakerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "closed"),
            Self::Open => write!(f, "open"),
            Self::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures required to open the circuit
    pub failure_threshold: u64,
    /// Number of consecutive successes required to close the circuit from half-open
    pub success_threshold: u64,
    /// Time to wait before transitioning from open to half-open
    pub recovery_timeout: Duration,
    /// Maximum number of requests allowed in half-open state
    pub half_open_max_requests: u64,
    /// Time window for counting failures (sliding window)
    pub failure_window: Duration,
    /// Minimum number of requests before circuit can open
    pub minimum_requests: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            recovery_timeout: Duration::from_secs(30),
            half_open_max_requests: 3,
            failure_window: Duration::from_secs(60),
            minimum_requests: 10,
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: CircuitBreakerState,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rejected_requests: u64,
    pub state_changes: u64,
    pub last_failure_time: Option<Instant>,
    pub last_success_time: Option<Instant>,
    pub current_failure_rate: f64,
    pub time_in_current_state: Duration,
}

/// Request outcome for circuit breaker tracking
#[derive(Debug, Clone, PartialEq)]
pub enum RequestOutcome {
    Success,
    Failure,
    Rejected,
}

/// Circuit breaker implementation with sliding window failure tracking
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerState>>,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    total_requests: AtomicU64,
    rejected_requests: AtomicU64,
    state_changes: AtomicU64,
    half_open_requests: AtomicU64,
    last_failure_time: Arc<Mutex<Option<Instant>>>,
    last_success_time: Arc<Mutex<Option<Instant>>>,
    state_change_time: Arc<Mutex<Instant>>,
    failure_window: Arc<Mutex<Vec<Instant>>>,
}
impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new<S: Into<String>>(name: S, config: CircuitBreakerConfig) -> Self {
        let name = name.into();
        info!(
            "🔌 Creating circuit breaker: {} with config: {:?}",
            name, config
        );

        Self {
            name,
            config,
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            rejected_requests: AtomicU64::new(0),
            state_changes: AtomicU64::new(0),
            half_open_requests: AtomicU64::new(0),
            last_failure_time: Arc::new(Mutex::new(None)),
            last_success_time: Arc::new(Mutex::new(None)),
            state_change_time: Arc::new(Mutex::new(Instant::now())),
            failure_window: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Check if a request can be executed
    pub async fn can_execute(&self) -> bool {
        let state = self.state.read().await;

        match *state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if recovery timeout has passed
                if let Some(last_failure) = *self.last_failure_time.lock().await {
                    if last_failure.elapsed() >= self.config.recovery_timeout {
                        drop(state);
                        self.transition_to_half_open().await;
                        return true;
                    }
                }
                false
            }
            CircuitBreakerState::HalfOpen => {
                // Allow limited requests in half-open state
                let current_requests = self.half_open_requests.load(Ordering::Relaxed);
                current_requests < self.config.half_open_max_requests
            }
        }
    }

    /// Execute a request with circuit breaker protection
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        if !self.can_execute().await {
            self.rejected_requests.fetch_add(1, Ordering::Relaxed);
            return Err(CircuitBreakerError::CircuitOpen {
                name: self.name.clone(),
                state: self.get_state().await,
            });
        }

        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let state = self.get_state().await;
        if state == CircuitBreakerState::HalfOpen {
            self.half_open_requests.fetch_add(1, Ordering::Relaxed);
        }

        match operation.await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(error) => {
                self.record_failure().await;
                Err(CircuitBreakerError::OperationFailed { error })
            }
        }
    }

    /// Record a successful request
    pub async fn record_success(&self) {
        let state = self.state.read().await;
        *self.last_success_time.lock().await = Some(Instant::now());

        match *state {
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
                self.success_count.fetch_add(1, Ordering::Relaxed);
            }
            CircuitBreakerState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if success_count >= self.config.success_threshold {
                    drop(state);
                    self.transition_to_closed().await;
                }
            }
            CircuitBreakerState::Open => {
                // This shouldn't happen, but handle it gracefully
                warn!(
                    "🔌 Recorded success while circuit breaker {} is open",
                    self.name
                );
            }
        }

        debug!("✅ Circuit breaker {} recorded success", self.name);
    }

    /// Record a failed request
    pub async fn record_failure(&self) {
        *self.last_failure_time.lock().await = Some(Instant::now());
        self.failure_count.fetch_add(1, Ordering::Relaxed);

        // Add failure to sliding window
        let mut window = self.failure_window.lock().await;
        let now = Instant::now();
        window.push(now);

        // Remove old failures outside the window
        window
            .retain(|&failure_time| now.duration_since(failure_time) <= self.config.failure_window);

        let state = self.state.read().await;
        match *state {
            CircuitBreakerState::Closed => {
                let total_requests = self.total_requests.load(Ordering::Relaxed);
                let failures_in_window = window.len() as u64;

                // Only consider opening if we have enough requests
                if total_requests >= self.config.minimum_requests
                    && failures_in_window >= self.config.failure_threshold
                {
                    drop(state);
                    drop(window);
                    self.transition_to_open().await;
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Any failure in half-open state should open the circuit
                drop(state);
                drop(window);
                self.transition_to_open().await;
            }
            CircuitBreakerState::Open => {
                // Already open, nothing to do
            }
        }

        debug!("❌ Circuit breaker {} recorded failure", self.name);
    }

    /// Get current circuit breaker state
    pub async fn get_state(&self) -> CircuitBreakerState {
        self.state.read().await.clone()
    }

    /// Get comprehensive statistics
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        let state = self.get_state().await;
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let successful_requests = self.success_count.load(Ordering::Relaxed);
        let failed_requests = self.failure_count.load(Ordering::Relaxed);
        let rejected_requests = self.rejected_requests.load(Ordering::Relaxed);

        let failure_rate = if total_requests > 0 {
            failed_requests as f64 / total_requests as f64
        } else {
            0.0
        };

        let time_in_current_state = self.state_change_time.lock().await.elapsed();

        CircuitBreakerStats {
            state,
            total_requests,
            successful_requests,
            failed_requests,
            rejected_requests,
            state_changes: self.state_changes.load(Ordering::Relaxed),
            last_failure_time: *self.last_failure_time.lock().await,
            last_success_time: *self.last_success_time.lock().await,
            current_failure_rate: failure_rate,
            time_in_current_state,
        }
    }

    /// Reset circuit breaker to initial state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Closed;

        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.half_open_requests.store(0, Ordering::Relaxed);

        *self.last_failure_time.lock().await = None;
        *self.last_success_time.lock().await = None;
        *self.state_change_time.lock().await = Instant::now();

        self.failure_window.lock().await.clear();

        info!("🔄 Circuit breaker {} reset to closed state", self.name);
    }

    /// Force circuit breaker to open state (for testing/emergency)
    pub async fn force_open(&self) {
        self.transition_to_open().await;
        warn!("🚨 Circuit breaker {} forced to open state", self.name);
    }

    /// Force circuit breaker to closed state (for testing/recovery)
    pub async fn force_close(&self) {
        self.transition_to_closed().await;
        info!("🔧 Circuit breaker {} forced to closed state", self.name);
    }

    /// Transition to closed state
    async fn transition_to_closed(&self) {
        let mut state = self.state.write().await;
        let old_state = state.clone();
        *state = CircuitBreakerState::Closed;

        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.half_open_requests.store(0, Ordering::Relaxed);
        self.state_changes.fetch_add(1, Ordering::Relaxed);
        *self.state_change_time.lock().await = Instant::now();

        info!(
            "✅ Circuit breaker {} transitioned from {} to closed",
            self.name, old_state
        );
    }

    /// Transition to open state
    async fn transition_to_open(&self) {
        let mut state = self.state.write().await;
        let old_state = state.clone();
        *state = CircuitBreakerState::Open;

        self.success_count.store(0, Ordering::Relaxed);
        self.half_open_requests.store(0, Ordering::Relaxed);
        self.state_changes.fetch_add(1, Ordering::Relaxed);
        *self.state_change_time.lock().await = Instant::now();

        error!(
            "🚨 Circuit breaker {} transitioned from {} to open",
            self.name, old_state
        );
    }

    /// Transition to half-open state
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        let old_state = state.clone();
        *state = CircuitBreakerState::HalfOpen;

        self.success_count.store(0, Ordering::Relaxed);
        self.half_open_requests.store(0, Ordering::Relaxed);
        self.state_changes.fetch_add(1, Ordering::Relaxed);
        *self.state_change_time.lock().await = Instant::now();

        info!(
            "🔄 Circuit breaker {} transitioned from {} to half-open",
            self.name, old_state
        );
    }

    /// Get the circuit breaker name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the circuit breaker configuration
    pub fn config(&self) -> &CircuitBreakerConfig {
        &self.config
    }
}

/// Circuit breaker specific errors
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker {name} is {state} - request rejected")]
    CircuitOpen {
        name: String,
        state: CircuitBreakerState,
    },

    #[error("Operation failed: {error}")]
    OperationFailed { error: E },
}

impl<E> From<CircuitBreakerError<E>> for RavenError
where
    E: std::fmt::Display,
{
    fn from(error: CircuitBreakerError<E>) -> Self {
        match error {
            CircuitBreakerError::CircuitOpen { name, state } => {
                RavenError::circuit_breaker_open(format!("Circuit breaker {name} is {state}"))
            }
            CircuitBreakerError::OperationFailed { error } => {
                RavenError::internal(error.to_string())
            }
        }
    }
}

/// Circuit breaker registry for managing multiple circuit breakers
pub struct CircuitBreakerRegistry {
    breakers: Arc<RwLock<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
}

impl CircuitBreakerRegistry {
    /// Create a new circuit breaker registry
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Register a circuit breaker
    pub async fn register(&self, breaker: Arc<CircuitBreaker>) {
        let mut breakers = self.breakers.write().await;
        let name = breaker.name().to_string();
        breakers.insert(name.clone(), breaker);
        info!("📋 Registered circuit breaker: {}", name);
    }

    /// Get a circuit breaker by name
    pub async fn get(&self, name: &str) -> Option<Arc<CircuitBreaker>> {
        let breakers = self.breakers.read().await;
        breakers.get(name).cloned()
    }

    /// Get or create a circuit breaker with default configuration
    pub async fn get_or_create(&self, name: &str) -> Arc<CircuitBreaker> {
        if let Some(breaker) = self.get(name).await {
            return breaker;
        }

        let breaker = Arc::new(CircuitBreaker::new(name, CircuitBreakerConfig::default()));
        self.register(Arc::clone(&breaker)).await;
        breaker
    }

    /// Get or create a circuit breaker with custom configuration
    pub async fn get_or_create_with_config(
        &self,
        name: &str,
        config: CircuitBreakerConfig,
    ) -> Arc<CircuitBreaker> {
        if let Some(breaker) = self.get(name).await {
            return breaker;
        }

        let breaker = Arc::new(CircuitBreaker::new(name, config));
        self.register(Arc::clone(&breaker)).await;
        breaker
    }

    /// Get all circuit breaker names
    pub async fn list_names(&self) -> Vec<String> {
        let breakers = self.breakers.read().await;
        breakers.keys().cloned().collect()
    }

    /// Get statistics for all circuit breakers
    pub async fn get_all_stats(&self) -> std::collections::HashMap<String, CircuitBreakerStats> {
        let breakers = self.breakers.read().await;
        let mut stats = std::collections::HashMap::new();

        for (name, breaker) in breakers.iter() {
            stats.insert(name.clone(), breaker.get_stats().await);
        }

        stats
    }

    /// Remove a circuit breaker
    pub async fn remove(&self, name: &str) -> bool {
        let mut breakers = self.breakers.write().await;
        let removed = breakers.remove(name).is_some();
        if removed {
            info!("🗑️ Removed circuit breaker: {}", name);
        }
        removed
    }

    /// Reset all circuit breakers
    pub async fn reset_all(&self) {
        let breakers = self.breakers.read().await;
        for breaker in breakers.values() {
            breaker.reset().await;
        }
        info!("🔄 Reset all circuit breakers");
    }
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

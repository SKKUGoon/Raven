// Circuit Breaker Tests - Project Raven
// "Testing the circuit that breaks before we do"

use super::*;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_circuit_breaker_creation() {
    let config = CircuitBreakerConfig::default();
    let breaker = CircuitBreaker::new("test", config);

    assert_eq!(breaker.name(), "test");
    assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);
    assert!(breaker.can_execute().await);
}

#[tokio::test]
async fn test_circuit_breaker_success() {
    let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

    breaker.record_success().await;

    let stats = breaker.get_stats().await;
    assert_eq!(stats.successful_requests, 1);
    assert_eq!(stats.state, CircuitBreakerState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_failure() {
    let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

    breaker.record_failure().await;

    let stats = breaker.get_stats().await;
    assert_eq!(stats.failed_requests, 1);
}

#[tokio::test]
async fn test_circuit_breaker_opens_on_failures() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        minimum_requests: 1,
        ..CircuitBreakerConfig::default()
    };

    let breaker = CircuitBreaker::new("test", config);

    // Record enough requests to meet minimum
    for _ in 0..5 {
        breaker.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    // Record failures to trigger opening
    for _ in 0..3 {
        breaker.record_failure().await;
    }

    assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);
    assert!(!breaker.can_execute().await);
}

#[tokio::test]
async fn test_circuit_breaker_half_open_transition() {
    let config = CircuitBreakerConfig {
        recovery_timeout: Duration::from_millis(10),
        failure_threshold: 1,
        minimum_requests: 1,
        ..CircuitBreakerConfig::default()
    };

    let breaker = CircuitBreaker::new("test", config);

    // Record a failure first to set the last_failure_time
    breaker.record_failure().await;

    // Force to open state
    breaker.force_open().await;
    assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);

    // Wait for recovery timeout
    sleep(Duration::from_millis(20)).await;

    // Should transition to half-open when checking
    assert!(breaker.can_execute().await);
    assert_eq!(breaker.get_state().await, CircuitBreakerState::HalfOpen);
}

#[tokio::test]
async fn test_circuit_breaker_execute_success() {
    let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

    let result = breaker.execute(async { Ok::<i32, &str>(42) }).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);

    let stats = breaker.get_stats().await;
    assert_eq!(stats.successful_requests, 1);
    assert_eq!(stats.total_requests, 1);
}

#[tokio::test]
async fn test_circuit_breaker_execute_failure() {
    let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

    let result = breaker
        .execute(async { Err::<i32, &str>("test error") })
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CircuitBreakerError::OperationFailed { error } => {
            assert_eq!(error, "test error");
        }
        _ => panic!("Expected OperationFailed error"),
    }

    let stats = breaker.get_stats().await;
    assert_eq!(stats.failed_requests, 1);
    assert_eq!(stats.total_requests, 1);
}

#[tokio::test]
async fn test_circuit_breaker_execute_rejected() {
    let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

    // Force to open state
    breaker.force_open().await;

    let result = breaker.execute(async { Ok::<i32, &str>(42) }).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CircuitBreakerError::CircuitOpen { name, state } => {
            assert_eq!(name, "test");
            assert_eq!(state, CircuitBreakerState::Open);
        }
        _ => panic!("Expected CircuitOpen error"),
    }

    let stats = breaker.get_stats().await;
    assert_eq!(stats.rejected_requests, 1);
}

#[tokio::test]
async fn test_circuit_breaker_registry() {
    let registry = CircuitBreakerRegistry::new();

    let breaker = registry.get_or_create("test").await;
    assert_eq!(breaker.name(), "test");

    let same_breaker = registry.get("test").await;
    assert!(same_breaker.is_some());

    let names = registry.list_names().await;
    assert!(names.contains(&"test".to_string()));

    assert!(registry.remove("test").await);
    assert!(registry.get("test").await.is_none());
}

#[tokio::test]
async fn test_circuit_breaker_reset() {
    let breaker = CircuitBreaker::new("test", CircuitBreakerConfig::default());

    breaker.record_failure().await;
    breaker.force_open().await;

    assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);

    breaker.reset().await;

    assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);
    let stats = breaker.get_stats().await;
    assert_eq!(stats.failed_requests, 0);
}

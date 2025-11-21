use raven::server::database::circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerState,
};
use std::time::Duration;

#[tokio::test]
async fn test_circuit_breaker() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 1,
        recovery_timeout: Duration::from_millis(50),
        half_open_max_requests: 1,
        failure_window: Duration::from_secs(1),
        minimum_requests: 2,
    };

    let breaker = CircuitBreaker::new("test_breaker", config);

    for _ in 0..2 {
        let result: Result<(), CircuitBreakerError<&str>> =
            breaker.execute(async { Err("fail") }).await;
        assert!(matches!(
            result,
            Err(CircuitBreakerError::OperationFailed { .. })
        ));
    }

    assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);
    assert!(!breaker.can_execute().await);
}

#[tokio::test]
async fn test_circuit_breaker_recovery() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 1,
        recovery_timeout: Duration::from_millis(20),
        half_open_max_requests: 1,
        failure_window: Duration::from_secs(1),
        minimum_requests: 2,
    };

    let breaker = CircuitBreaker::new("test_breaker", config);

    for _ in 0..2 {
        let _ = breaker.execute(async { Err::<(), _>("fail") }).await;
    }

    assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);

    tokio::time::sleep(Duration::from_millis(25)).await;

    assert!(breaker.can_execute().await);
    assert_eq!(breaker.get_state().await, CircuitBreakerState::HalfOpen);

    let result: Result<(), CircuitBreakerError<&str>> =
        breaker.execute(async { Ok::<(), &str>(()) }).await;
    assert!(result.is_ok());

    assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);
}

//! Circuit breaker pattern for preventing cascading failures.
//!
//! This module implements a circuit breaker that opens after consecutive failures,
//! preventing requests to a failing service, and allowing it to recover.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    /// Circuit is closed, requests pass through normally
    Closed = 0,
    /// Circuit is open, requests fail fast
    Open = 1,
    /// Circuit is half-open, one request allowed to test recovery
    HalfOpen = 2,
}

impl State {
    fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(State::Closed),
            1 => Some(State::Open),
            2 => Some(State::HalfOpen),
            _ => None,
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    pub failure_threshold: u32,
    /// Number of consecutive successes needed to close circuit in HalfOpen state
    pub success_threshold: u32,
    /// How long to wait before attempting recovery (Open -> HalfOpen)
    pub open_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            open_timeout: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker for preventing cascading failures
pub struct CircuitBreaker {
    state: Arc<AtomicU8>,
    failure_count: Arc<AtomicU32>,
    success_count: Arc<AtomicU32>,
    last_failure_time: Arc<std::sync::Mutex<Option<Instant>>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(AtomicU8::new(State::Closed as u8)),
            failure_count: Arc::new(AtomicU32::new(0)),
            success_count: Arc::new(AtomicU32::new(0)),
            last_failure_time: Arc::new(std::sync::Mutex::new(None)),
            config,
        }
    }

    /// Create a circuit breaker with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Get the current state
    pub fn state(&self) -> State {
        State::from_u8(self.state.load(Ordering::Acquire)).unwrap_or(State::Closed)
    }

    /// Check if a request is allowed (circuit is not open)
    pub fn allow_request(&self) -> bool {
        let current_state = self.state();

        match current_state {
            State::Closed => true,
            State::Open => {
                // Check if we should transition to HalfOpen
                let last_failure = self.last_failure_time.lock().ok().and_then(|guard| *guard);
                if let Some(failure_time) = last_failure {
                    if failure_time.elapsed() >= self.config.open_timeout {
                        debug!("Circuit breaker transitioning from Open to HalfOpen");
                        self.set_state(State::HalfOpen);
                        self.success_count.store(0, Ordering::Release);
                        return true;
                    }
                }
                warn!("Circuit breaker is OPEN, rejecting request");
                false
            }
            State::HalfOpen => {
                // Allow one request through to test recovery
                debug!("Circuit breaker is HALF-OPEN, allowing test request");
                true
            }
        }
    }

    /// Record a successful request
    pub fn record_success(&self) {
        let current_state = self.state();

        match current_state {
            State::Closed => {
                // Reset failure count on success in Closed state
                self.failure_count.store(0, Ordering::Release);
            }
            State::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Release) + 1;
                debug!(
                    "Circuit breaker success in HalfOpen: {}/{}",
                    success_count, self.config.success_threshold
                );

                if success_count >= self.config.success_threshold {
                    debug!("Circuit breaker closing after reaching success threshold");
                    self.set_state(State::Closed);
                    self.failure_count.store(0, Ordering::Release);
                    self.success_count.store(0, Ordering::Release);
                }
            }
            State::Open => {
                // Shouldn't happen, but handle gracefully
                debug!("Recorded success while in Open state, transitioning to HalfOpen");
                self.set_state(State::HalfOpen);
                self.success_count.store(1, Ordering::Release);
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        let failure_count = self.failure_count.fetch_add(1, Ordering::Release) + 1;
        if let Ok(mut last_failure) = self.last_failure_time.lock() {
            *last_failure = Some(Instant::now());
        }

        let current_state = self.state();

        match current_state {
            State::Closed => {
                warn!(
                    "Circuit breaker failure in Closed: {}/{}",
                    failure_count, self.config.failure_threshold
                );

                if failure_count >= self.config.failure_threshold {
                    warn!("Circuit breaker opening after reaching failure threshold");
                    self.set_state(State::Open);
                    self.success_count.store(0, Ordering::Release);
                }
            }
            State::HalfOpen => {
                warn!("Circuit breaker failure in HalfOpen, reopening circuit");
                self.set_state(State::Open);
                self.success_count.store(0, Ordering::Release);
            }
            State::Open => {
                // Already open, just update failure count
                debug!("Circuit breaker failure in Open state");
            }
        }
    }

    /// Reset the circuit breaker to Closed state
    pub fn reset(&self) {
        debug!("Resetting circuit breaker to Closed state");
        self.set_state(State::Closed);
        self.failure_count.store(0, Ordering::Release);
        self.success_count.store(0, Ordering::Release);
        if let Ok(mut last_failure) = self.last_failure_time.lock() {
            *last_failure = None;
        }
    }

    fn set_state(&self, new_state: State) {
        self.state.store(new_state as u8, Ordering::Release);
    }
}

impl Clone for CircuitBreaker {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            failure_count: Arc::clone(&self.failure_count),
            success_count: Arc::clone(&self.success_count),
            last_failure_time: Arc::clone(&self.last_failure_time),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_opens_on_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            open_timeout: Duration::from_millis(100),
        });

        assert_eq!(cb.state(), State::Closed);
        assert!(cb.allow_request());

        for _ in 0..3 {
            cb.record_failure();
        }

        assert_eq!(cb.state(), State::Open);
        assert!(!cb.allow_request());
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            open_timeout: Duration::from_millis(50),
        });

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), State::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(60)).await;

        // First request should be allowed (HalfOpen)
        assert!(cb.allow_request());
        assert_eq!(cb.state(), State::HalfOpen);

        // Record success
        cb.record_success();
        assert_eq!(cb.state(), State::HalfOpen);

        // Second success closes circuit
        cb.record_success();
        assert_eq!(cb.state(), State::Closed);
    }
}

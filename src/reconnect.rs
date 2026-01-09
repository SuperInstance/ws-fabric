//! Reconnection logic with exponential backoff

use crate::{config::ReconnectConfig, error::Result};
use std::time::{Duration, Instant};

/// Reconnection state machine
#[derive(Debug, Clone)]
pub struct ReconnectState {
    config: ReconnectConfig,
    attempt: u32,
    last_attempt: Option<Instant>,
    current_delay: Duration,
}

impl ReconnectState {
    /// Create a new reconnection state
    pub fn new(config: ReconnectConfig) -> Self {
        let initial_delay = config.initial_delay;
        Self {
            config,
            attempt: 0,
            last_attempt: None,
            current_delay: initial_delay,
        }
    }

    /// Check if reconnection is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if should attempt reconnection
    pub fn should_reconnect(&self) -> bool {
        if !self.is_enabled() {
            return false;
        }

        if self.config.max_attempts > 0 && self.attempt >= self.config.max_attempts {
            return false;
        }

        true
    }

    /// Calculate delay before next reconnection attempt
    pub fn next_delay(&self) -> Duration {
        self.current_delay
    }

    /// Record a reconnection attempt
    pub fn record_attempt(&mut self) {
        self.attempt += 1;
        self.last_attempt = Some(Instant::now());

        // Calculate next delay with exponential backoff
        self.current_delay = Duration::from_millis(
            (self.current_delay.as_millis() as f64 * self.config.backoff_multiplier).min(
                self.config.max_delay.as_millis() as f64,
            ) as u64,
        );
    }

    /// Reset reconnection state
    pub fn reset(&mut self) {
        self.attempt = 0;
        self.last_attempt = None;
        self.current_delay = self.config.initial_delay;
    }

    /// Get current attempt count
    pub fn attempts(&self) -> u32 {
        self.attempt
    }

    /// Wait for next reconnection attempt
    pub async fn wait_for_next_attempt(&self) -> Result<()> {
        if !self.should_reconnect() {
            return Err(crate::error::Error::ReconnectTimeout);
        }

        tokio::time::sleep(self.next_delay()).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconnect_state_initial() {
        let config = ReconnectConfig::new();
        let state = ReconnectState::new(config);

        assert!(state.is_enabled());
        assert_eq!(state.attempts(), 0);
        assert_eq!(state.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn test_reconnect_state_disabled() {
        let config = ReconnectConfig::disabled();
        let state = ReconnectState::new(config);

        assert!(!state.is_enabled());
        assert!(!state.should_reconnect());
    }

    #[test]
    fn test_reconnect_state_max_attempts() {
        let config = ReconnectConfig::new().with_max_attempts(3);
        let mut state = ReconnectState::new(config);

        assert!(state.should_reconnect());
        state.record_attempt();
        assert!(state.should_reconnect());
        state.record_attempt();
        assert!(state.should_reconnect());
        state.record_attempt();
        assert!(!state.should_reconnect());
    }

    #[test]
    fn test_reconnect_state_exponential_backoff() {
        let config = ReconnectConfig::new()
            .with_initial_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(10))
            .with_backoff_multiplier(2.0);

        let mut state = ReconnectState::new(config);

        assert_eq!(state.next_delay(), Duration::from_millis(100));
        state.record_attempt();
        assert_eq!(state.next_delay(), Duration::from_millis(200));
        state.record_attempt();
        assert_eq!(state.next_delay(), Duration::from_millis(400));
        state.record_attempt();
        assert_eq!(state.next_delay(), Duration::from_millis(800));
    }

    #[test]
    fn test_reconnect_state_max_delay() {
        let config = ReconnectConfig::new()
            .with_initial_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(2))
            .with_backoff_multiplier(10.0);

        let mut state = ReconnectState::new(config);

        assert_eq!(state.next_delay(), Duration::from_secs(1));
        state.record_attempt();
        // Should be capped at max_delay
        assert_eq!(state.next_delay(), Duration::from_secs(2));
    }

    #[test]
    fn test_reconnect_state_reset() {
        let config = ReconnectConfig::new();
        let mut state = ReconnectState::new(config);

        state.record_attempt();
        state.record_attempt();
        assert_eq!(state.attempts(), 2);

        state.reset();
        assert_eq!(state.attempts(), 0);
        assert_eq!(state.next_delay(), Duration::from_millis(100));
    }
}

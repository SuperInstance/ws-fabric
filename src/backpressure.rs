//! Backpressure control for WebSocket connections

use crate::config::BackpressureConfig;
use parking_lot::Mutex;
use std::sync::Arc;

/// Backpressure controller
#[derive(Debug, Clone)]
pub struct BackpressureController {
    config: BackpressureConfig,
    state: Arc<Mutex<BackpressureState>>,
}

#[derive(Debug)]
struct BackpressureState {
    current_size: usize,
}

impl BackpressureController {
    /// Create a new backpressure controller
    pub fn new(config: BackpressureConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(BackpressureState { current_size: 0 })),
        }
    }

    /// Check if backpressure is currently active
    pub fn is_backpressure_active(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        let state = self.state.lock();
        state.current_size > (self.config.max_buffer_size as f64 * self.config.backpressure_threshold) as usize
    }

    /// Check if should send (not under backpressure)
    pub fn can_send(&self) -> bool {
        !self.is_backpressure_active()
    }

    /// Record a send operation
    pub fn record_send(&self, size: usize) {
        let mut state = self.state.lock();
        state.current_size += size;
    }

    /// Record a receive operation (buffer space freed)
    pub fn record_recv(&self, size: usize) {
        let mut state = self.state.lock();
        state.current_size = state.current_size.saturating_sub(size);
    }

    /// Get current buffer size
    pub fn current_size(&self) -> usize {
        let state = self.state.lock();
        state.current_size
    }

    /// Get buffer utilization ratio (0.0-1.0)
    pub fn utilization(&self) -> f64 {
        let state = self.state.lock();
        state.current_size as f64 / self.config.max_buffer_size as f64
    }

    /// Wait for backpressure to clear
    pub async fn wait_for_backpressure_clear(&self) -> crate::error::Result<()> {
        while self.is_backpressure_active() {
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Check if we've waited too long (timeout after 30 seconds)
            // In real implementation, this should use proper timeout
        }

        Ok(())
    }

    /// Check if buffer is full
    pub fn is_full(&self) -> bool {
        let state = self.state.lock();
        state.current_size >= self.config.max_buffer_size
    }
}

use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backpressure_disabled() {
        let config = BackpressureConfig::disabled();
        let controller = BackpressureController::new(config);

        assert!(!controller.is_backpressure_active());
        assert!(controller.can_send());
    }

    #[test]
    fn test_backpressure_activation() {
        let config = BackpressureConfig::new()
            .with_max_buffer_size(100)
            .with_backpressure_threshold(0.8);

        let controller = BackpressureController::new(config);

        assert!(!controller.is_backpressure_active());

        // Add 81 bytes (above 80% threshold)
        controller.record_send(81);

        assert!(controller.is_backpressure_active());
        assert!(!controller.can_send());
    }

    #[test]
    fn test_backpressure_recovery() {
        let config = BackpressureConfig::new()
            .with_max_buffer_size(100)
            .with_backpressure_threshold(0.8)
            .with_recovery_threshold(0.6);

        let controller = BackpressureController::new(config);

        // Add 81 bytes (above 80% threshold)
        controller.record_send(81);
        assert!(controller.is_backpressure_active());

        // Receive 21 bytes (down to 60, at recovery threshold)
        controller.record_recv(21);
        assert!(!controller.is_backpressure_active());
    }

    #[test]
    fn test_buffer_full() {
        let config = BackpressureConfig::new()
            .with_max_buffer_size(100);

        let controller = BackpressureController::new(config);

        assert!(!controller.is_full());

        controller.record_send(100);
        assert!(controller.is_full());
    }

    #[test]
    fn test_utilization() {
        let config = BackpressureConfig::new()
            .with_max_buffer_size(1000);

        let controller = BackpressureController::new(config);

        assert_eq!(controller.utilization(), 0.0);

        controller.record_send(500);
        assert_eq!(controller.utilization(), 0.5);

        controller.record_send(250);
        assert_eq!(controller.utilization(), 0.75);
    }
}

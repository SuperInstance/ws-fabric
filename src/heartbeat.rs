//! Heartbeat/ping-pong keepalive for WebSocket connections

use crate::{config::HeartbeatConfig, error::Result, Message};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::Interval;

/// Heartbeat manager
#[derive(Debug, Clone)]
pub struct HeartbeatManager {
    config: HeartbeatConfig,
    state: Arc<Mutex<HeartbeatState>>,
}

#[derive(Debug)]
struct HeartbeatState {
    last_ping: Option<Instant>,
    last_pong: Option<Instant>,
    ping_count: u64,
    pong_count: u64,
    missed_pings: u8,
}

impl HeartbeatManager {
    /// Create a new heartbeat manager
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(HeartbeatState {
                last_ping: None,
                last_pong: None,
                ping_count: 0,
                pong_count: 0,
                missed_pings: 0,
            })),
        }
    }

    /// Check if heartbeat is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Generate a ping message
    pub fn ping(&self) -> Message {
        let payload = self.ping_count().to_be_bytes().to_vec();
        let mut state = self.state.lock();
        state.last_ping = Some(Instant::now());
        state.ping_count += 1;

        Message::ping(payload)
    }

    /// Handle a pong message
    pub fn handle_pong(&self, payload: &[u8]) -> Result<()> {
        let mut state = self.state.lock();

        // Check if pong matches our ping
        if payload.len() == 8 {
            let ping_num = u64::from_be_bytes(payload.try_into().unwrap());
            // ping() increments count AFTER generating payload, so pong should match (count - 1)
            // unless count is 0 (which shouldn't happen)
            let expected_ping = if state.ping_count > 0 { state.ping_count - 1 } else { 0 };
            if ping_num == expected_ping {
                state.last_pong = Some(Instant::now());
                state.pong_count += 1;
                state.missed_pings = 0;
                return Ok(());
            }
        }

        // Pong doesn't match
        state.missed_pings += 1;

        if state.missed_pings > 3 {
            Err(crate::error::Error::Timeout)
        } else {
            Ok(())
        }
    }

    /// Check if connection is alive (pong received recently)
    pub fn is_alive(&self) -> bool {
        if !self.is_enabled() {
            return true; // Assume alive if heartbeat disabled
        }

        let state = self.state.lock();

        if let Some(last_pong) = state.last_pong {
            last_pong.elapsed() < self.config.ping_timeout
        } else {
            // No pong yet, check if we've sent a ping recently
            if let Some(last_ping) = state.last_ping {
                last_ping.elapsed() < self.config.ping_timeout
            } else {
                true // Haven't sent any pings yet
            }
        }
    }

    /// Get number of pings sent
    pub fn ping_count(&self) -> u64 {
        let state = self.state.lock();
        state.ping_count
    }

    /// Get number of pongs received
    pub fn pong_count(&self) -> u64 {
        let state = self.state.lock();
        state.pong_count
    }

    /// Reset heartbeat state
    pub fn reset(&self) {
        let mut state = self.state.lock();
        state.last_ping = None;
        state.last_pong = None;
        state.missed_pings = 0;
    }

    /// Create ping interval stream
    pub fn ping_interval(&self) -> Result<Interval> {
        if !self.is_enabled() {
            return Err(crate::error::Error::not_supported("Heartbeat is disabled"));
        }

        Ok(tokio::time::interval(self.config.ping_interval))
    }

    /// Handle a ping message (server-side)
    pub fn handle_ping(&self, payload: &[u8]) -> Message {
        Message::pong(payload.to_vec())
    }

    /// Get time since last pong
    pub fn time_since_last_pong(&self) -> Option<Duration> {
        let state = self.state.lock();
        state.last_pong.map(|t| t.elapsed())
    }

    /// Get missed ping count
    pub fn missed_pings(&self) -> u8 {
        let state = self.state.lock();
        state.missed_pings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_disabled() {
        let config = HeartbeatConfig::disabled();
        let manager = HeartbeatManager::new(config);

        assert!(!manager.is_enabled());
        assert!(manager.is_alive()); // Always alive when disabled
    }

    #[test]
    fn test_heartbeat_ping() {
        let config = HeartbeatConfig::new();
        let manager = HeartbeatManager::new(config);

        let ping = manager.ping();
        assert!(ping.is_ping());
        assert_eq!(manager.ping_count(), 1);
    }

    #[test]
    fn test_heartbeat_handle_pong() {
        let config = HeartbeatConfig::new();
        let manager = HeartbeatManager::new(config);

        let ping = manager.ping();
        let payload = ping.payload();

        manager.handle_pong(payload).unwrap();
        assert_eq!(manager.pong_count(), 1);
        assert!(manager.is_alive());
    }

    #[test]
    fn test_heartbeat_missed_pings() {
        let config = HeartbeatConfig::new();
        let manager = HeartbeatManager::new(config);

        // Send a ping (ping_count becomes 1, payload was 0)
        manager.ping();

        // Handle wrong pong (different payload - use 255 instead of 0)
        let wrong_payload = [255u8; 8];
        manager.handle_pong(&wrong_payload).unwrap();
        assert_eq!(manager.missed_pings(), 1);

        // Handle more wrong pongs
        manager.handle_pong(&wrong_payload).unwrap();
        assert_eq!(manager.missed_pings(), 2);

        manager.handle_pong(&wrong_payload).unwrap();
        assert_eq!(manager.missed_pings(), 3);

        // Fourth one should trigger timeout
        let result = manager.handle_pong(&wrong_payload);
        assert!(matches!(result, Err(crate::error::Error::Timeout)));
    }

    #[test]
    fn test_heartbeat_is_alive() {
        let config = HeartbeatConfig::new();
        let manager = HeartbeatManager::new(config);

        // Initially alive (no pings sent yet)
        assert!(manager.is_alive());

        // Send ping
        let ping = manager.ping();

        // Still alive (within timeout)
        assert!(manager.is_alive());

        // Handle pong
        manager.handle_pong(ping.payload()).unwrap();
        assert!(manager.is_alive());

        // Check pong count
        assert_eq!(manager.pong_count(), 1);
    }

    #[test]
    fn test_heartbeat_handle_server_ping() {
        let config = HeartbeatConfig::new();
        let manager = HeartbeatManager::new(config);

        let ping_payload = b"server ping";
        let pong = manager.handle_ping(ping_payload);

        assert!(pong.is_pong());
        assert_eq!(pong.as_bytes(), ping_payload);
    }

    #[test]
    fn test_heartbeat_reset() {
        let config = HeartbeatConfig::new();
        let manager = HeartbeatManager::new(config);

        manager.ping();
        manager.ping();
        assert_eq!(manager.ping_count(), 2);

        manager.reset();
        assert_eq!(manager.ping_count(), 2); // reset doesn't clear counters

        // But clears last ping/pong
        assert!(manager.time_since_last_pong().is_none());
    }
}

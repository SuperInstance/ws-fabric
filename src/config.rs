//! Configuration structures for WebSocket client and server

use crate::{DEFAULT_BUFFER_SIZE, DEFAULT_CONNECTION_TIMEOUT, DEFAULT_MAX_MESSAGE_SIZE, DEFAULT_PING_INTERVAL, DEFAULT_PING_TIMEOUT};
use std::time::Duration;

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// WebSocket URL to connect to
    pub url: String,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Auto-reconnect on connection loss
    pub auto_reconnect: bool,
    /// Reconnection configuration
    pub reconnect_config: ReconnectConfig,
    /// Heartbeat/ping configuration
    pub heartbeat_config: HeartbeatConfig,
    /// Backpressure configuration
    pub backpressure_config: BackpressureConfig,
    /// Connection timeout
    pub connection_timeout: Duration,
}

impl ClientConfig {
    /// Create a new client configuration
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            auto_reconnect: true,
            reconnect_config: ReconnectConfig::default(),
            heartbeat_config: HeartbeatConfig::default(),
            backpressure_config: BackpressureConfig::default(),
            connection_timeout: Duration::from_secs(DEFAULT_CONNECTION_TIMEOUT),
        }
    }

    /// Set maximum message size
    pub fn with_max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    /// Enable/disable auto-reconnect
    pub fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// Set reconnection configuration
    pub fn with_reconnect_config(mut self, config: ReconnectConfig) -> Self {
        self.reconnect_config = config;
        self
    }

    /// Set heartbeat configuration
    pub fn with_heartbeat_config(mut self, config: HeartbeatConfig) -> Self {
        self.heartbeat_config = config;
        self
    }

    /// Set backpressure configuration
    pub fn with_backpressure_config(mut self, config: BackpressureConfig) -> Self {
        self.backpressure_config = config;
        self
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to bind to
    pub bind_address: String,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Maximum message size
    pub max_message_size: usize,
    /// Ping interval
    pub ping_interval: Duration,
    /// Ping timeout
    pub ping_timeout: Duration,
    /// Buffer size for message channels
    pub buffer_size: usize,
}

impl ServerConfig {
    /// Create a new server configuration
    pub fn new(bind_address: impl Into<String>) -> Self {
        Self {
            bind_address: bind_address.into(),
            max_connections: 10_000,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            ping_interval: Duration::from_secs(DEFAULT_PING_INTERVAL),
            ping_timeout: Duration::from_secs(DEFAULT_PING_TIMEOUT),
            buffer_size: DEFAULT_BUFFER_SIZE,
        }
    }

    /// Set maximum connections
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// Set maximum message size
    pub fn with_max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    /// Set ping interval
    pub fn with_ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// Set ping timeout
    pub fn with_ping_timeout(mut self, timeout: Duration) -> Self {
        self.ping_timeout = timeout;
        self
    }

    /// Set buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
}

/// Reconnection configuration
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Enable automatic reconnection
    pub enabled: bool,
    /// Maximum number of reconnection attempts (0 = infinite)
    pub max_attempts: u32,
    /// Initial delay before first reconnection attempt
    pub initial_delay: Duration,
    /// Maximum delay between reconnection attempts
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 0, // Infinite
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 1.5,
        }
    }
}

impl ReconnectConfig {
    /// Create a new reconnection configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable reconnection
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Set maximum attempts
    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_attempts = max;
        self
    }

    /// Set initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set maximum delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set backoff multiplier
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }
}

/// Heartbeat configuration
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Enable heartbeat/ping-pong
    pub enabled: bool,
    /// Ping interval
    pub ping_interval: Duration,
    /// Ping timeout (time to wait for pong)
    pub ping_timeout: Duration,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ping_interval: Duration::from_secs(DEFAULT_PING_INTERVAL),
            ping_timeout: Duration::from_secs(DEFAULT_PING_TIMEOUT),
        }
    }
}

impl HeartbeatConfig {
    /// Create a new heartbeat configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable heartbeat
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Set ping interval
    pub fn with_ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// Set ping timeout
    pub fn with_ping_timeout(mut self, timeout: Duration) -> Self {
        self.ping_timeout = timeout;
        self
    }
}

/// Backpressure configuration
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Enable backpressure control
    pub enabled: bool,
    /// Maximum buffer size before backpressure kicks in
    pub max_buffer_size: usize,
    /// Threshold (0.0-1.0) for activating backpressure
    pub backpressure_threshold: f64,
    /// Threshold (0.0-1.0) for deactivating backpressure
    pub recovery_threshold: f64,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_buffer_size: DEFAULT_BUFFER_SIZE,
            backpressure_threshold: 0.8,
            recovery_threshold: 0.6,
        }
    }
}

impl BackpressureConfig {
    /// Create a new backpressure configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable backpressure
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Set maximum buffer size
    pub fn with_max_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = size;
        self
    }

    /// Set backpressure threshold (0.0-1.0)
    pub fn with_backpressure_threshold(mut self, threshold: f64) -> Self {
        assert!((0.0..=1.0).contains(&threshold));
        self.backpressure_threshold = threshold;
        self
    }

    /// Set recovery threshold (0.0-1.0)
    pub fn with_recovery_threshold(mut self, threshold: f64) -> Self {
        assert!((0.0..=1.0).contains(&threshold));
        self.recovery_threshold = threshold;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_builder() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_max_message_size(2048)
            .with_auto_reconnect(false);

        assert_eq!(config.url, "ws://localhost:8080");
        assert_eq!(config.max_message_size, 2048);
        assert!(!config.auto_reconnect);
    }

    #[test]
    fn test_server_config_builder() {
        let config = ServerConfig::new("0.0.0.0:8080")
            .with_max_connections(5000)
            .with_buffer_size(2000);

        assert_eq!(config.bind_address, "0.0.0.0:8080");
        assert_eq!(config.max_connections, 5000);
        assert_eq!(config.buffer_size, 2000);
    }

    #[test]
    fn test_reconnect_config_defaults() {
        let config = ReconnectConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_attempts, 0); // Infinite
        assert_eq!(config.backoff_multiplier, 1.5);
    }

    #[test]
    fn test_heartbeat_config_defaults() {
        let config = HeartbeatConfig::default();
        assert!(config.enabled);
        assert_eq!(config.ping_interval.as_secs(), 30);
        assert_eq!(config.ping_timeout.as_secs(), 10);
    }

    #[test]
    fn test_backpressure_config_defaults() {
        let config = BackpressureConfig::default();
        assert!(config.enabled);
        assert_eq!(config.backpressure_threshold, 0.8);
        assert_eq!(config.recovery_threshold, 0.6);
    }

    #[test]
    fn test_backpressure_threshold_validation() {
        let result = std::panic::catch_unwind(|| {
            BackpressureConfig::new().with_backpressure_threshold(1.5);
        });
        assert!(result.is_err());
    }
}

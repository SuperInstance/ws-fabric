//! Configuration structures for WebSocket client and server

use crate::{DEFAULT_BUFFER_SIZE, DEFAULT_CONNECTION_TIMEOUT, DEFAULT_MAX_MESSAGE_SIZE, DEFAULT_PING_INTERVAL, DEFAULT_PING_TIMEOUT};
use crate::compression::CompressionConfig;
use crate::subprotocol::SubprotocolList;
use crate::headers::HeaderMap;
use crate::ratelimit::RateLimitConfig;
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
    /// Compression configuration
    pub compression_config: CompressionConfig,
    /// Rate limit configuration
    pub rate_limit_config: RateLimitConfig,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Subprotocol negotiation
    pub subprotocols: SubprotocolList,
    /// Custom HTTP headers for handshake
    pub headers: HeaderMap,
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
            compression_config: CompressionConfig::default(),
            rate_limit_config: RateLimitConfig::default(),
            connection_timeout: Duration::from_secs(DEFAULT_CONNECTION_TIMEOUT),
            subprotocols: SubprotocolList::new(),
            headers: HeaderMap::new(),
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

    /// Set compression configuration
    pub fn with_compression_config(mut self, config: CompressionConfig) -> Self {
        self.compression_config = config;
        self
    }

    /// Set rate limit configuration
    pub fn with_rate_limit_config(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit_config = config;
        self
    }

    /// Set subprotocols for negotiation
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ClientConfig;
    ///
    /// let config = ClientConfig::new("ws://localhost:8080")
    ///     .with_subprotocols(&["chat-v2", "mqtt"]);
    /// ```
    pub fn with_subprotocols(mut self, protocols: &[&str]) -> Self {
        self.subprotocols = SubprotocolList::from_slice(protocols).unwrap_or_else(|_| {
            // If validation fails, use empty list
            SubprotocolList::new()
        });
        self
    }

    /// Add a custom header to the handshake
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ClientConfig;
    ///
    /// let config = ClientConfig::new("ws://localhost:8080")
    ///     .with_header("Authorization", "Bearer token123")
    ///     .with_header("X-API-Key", "secret-key");
    /// ```
    pub fn with_header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let _ = self.headers.insert(name, value);
        self
    }

    /// Add multiple custom headers to the handshake
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ClientConfig;
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("Authorization", "Bearer token123").unwrap();
    /// headers.insert("X-API-Key", "secret-key").unwrap();
    ///
    /// let config = ClientConfig::new("ws://localhost:8080")
    ///     .with_headers(&headers);
    /// ```
    pub fn with_headers(mut self, headers: &HeaderMap) -> Self {
        let cloned = headers.clone();
        self.headers.extend(cloned);
        self
    }

    /// Set the Authorization header
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ClientConfig;
    ///
    /// let config = ClientConfig::new("ws://localhost:8080")
    ///     .with_authorization("Bearer token123");
    /// ```
    pub fn with_authorization(mut self, value: impl AsRef<str>) -> Self {
        let _ = self.headers.set_authorization(value);
        self
    }

    /// Set the User-Agent header
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ClientConfig;
    ///
    /// let config = ClientConfig::new("ws://localhost:8080")
    ///     .with_user_agent("MyClient/1.0");
    /// ```
    pub fn with_user_agent(mut self, value: impl AsRef<str>) -> Self {
        let _ = self.headers.set_user_agent(value);
        self
    }

    /// Set the Origin header
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ClientConfig;
    ///
    /// let config = ClientConfig::new("ws://localhost:8080")
    ///     .with_origin("https://example.com");
    /// ```
    pub fn with_origin(mut self, value: impl AsRef<str>) -> Self {
        let _ = self.headers.set_origin(value);
        self
    }

    /// Set the Cookie header
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ClientConfig;
    ///
    /// let config = ClientConfig::new("ws://localhost:8080")
    ///     .with_cookie("session=abc123; user=john");
    /// ```
    pub fn with_cookie(mut self, value: impl AsRef<str>) -> Self {
        let _ = self.headers.set_cookie(value);
        self
    }

    /// Set the X-API-Key header
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ClientConfig;
    ///
    /// let config = ClientConfig::new("ws://localhost:8080")
    ///     .with_api_key("secret-key-123");
    /// ```
    pub fn with_api_key(mut self, value: impl AsRef<str>) -> Self {
        let _ = self.headers.set_api_key(value);
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
    /// Compression configuration
    pub compression_config: CompressionConfig,
    /// Rate limit configuration
    pub rate_limit_config: RateLimitConfig,
    /// Subprotocol negotiation
    pub subprotocols: Vec<String>,
    /// Custom HTTP headers for handshake
    pub headers: HeaderMap,
    /// Required headers for connection (e.g., X-API-Key)
    pub required_headers: Vec<String>,
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
            compression_config: CompressionConfig::default(),
            rate_limit_config: RateLimitConfig::default(),
            subprotocols: Vec::new(),
            headers: HeaderMap::new(),
            required_headers: Vec::new(),
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

    /// Set compression configuration
    pub fn with_compression_config(mut self, config: CompressionConfig) -> Self {
        self.compression_config = config;
        self
    }

    /// Set rate limit configuration
    pub fn with_rate_limit_config(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit_config = config;
        self
    }

    /// Set subprotocols for negotiation
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ServerConfig;
    ///
    /// let config = ServerConfig::new("0.0.0.0:8080")
    ///     .with_subprotocols(&["mqtt", "chat-v2"]);
    /// ```
    pub fn with_subprotocols(mut self, protocols: &[&str]) -> Self {
        self.subprotocols = protocols.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add a custom header to the handshake
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ServerConfig;
    ///
    /// let config = ServerConfig::new("0.0.0.0:8080")
    ///     .with_header("X-Server", "MyServer/1.0")
    ///     .with_header("X-Custom", "value");
    /// ```
    pub fn with_header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let _ = self.headers.insert(name, value);
        self
    }

    /// Add multiple custom headers to the handshake
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ServerConfig;
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("X-Server", "MyServer/1.0").unwrap();
    /// headers.insert("X-Custom", "value").unwrap();
    ///
    /// let config = ServerConfig::new("0.0.0.0:8080")
    ///     .with_headers(&headers);
    /// ```
    pub fn with_headers(mut self, headers: &HeaderMap) -> Self {
        let cloned = headers.clone();
        self.headers.extend(cloned);
        self
    }

    /// Require a specific header to be present in client handshake
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::ServerConfig;
    ///
    /// let config = ServerConfig::new("0.0.0.0:8080")
    ///     .with_required_header("X-API-Key")
    ///     .with_required_header("Authorization");
    /// ```
    pub fn with_required_header(mut self, header: impl AsRef<str>) -> Self {
        self.required_headers.push(header.as_ref().to_string());
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

    #[test]
    fn test_client_config_with_compression() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_compression_config(CompressionConfig::new().with_level(9));

        assert_eq!(config.url, "ws://localhost:8080");
        assert_eq!(config.compression_config.level, 9);
    }

    #[test]
    fn test_server_config_with_compression() {
        let config = ServerConfig::new("0.0.0.0:8080")
            .with_compression_config(CompressionConfig::disabled());

        assert_eq!(config.bind_address, "0.0.0.0:8080");
        assert!(!config.compression_config.enabled);
    }

    #[test]
    fn test_client_config_with_subprotocols() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_subprotocols(&["chat-v2", "mqtt", "graphql"]);

        assert_eq!(config.url, "ws://localhost:8080");
        assert_eq!(config.subprotocols.len(), 3);
        assert!(config.subprotocols.contains("chat-v2"));
        assert!(config.subprotocols.contains("mqtt"));
        assert!(config.subprotocols.contains("graphql"));
    }

    #[test]
    fn test_client_config_empty_subprotocols() {
        let config = ClientConfig::new("ws://localhost:8080");

        assert_eq!(config.url, "ws://localhost:8080");
        assert!(config.subprotocols.is_empty());
    }

    #[test]
    fn test_server_config_with_subprotocols() {
        let config = ServerConfig::new("0.0.0.0:8080")
            .with_subprotocols(&["mqtt", "chat-v2", "chat-v1"]);

        assert_eq!(config.bind_address, "0.0.0.0:8080");
        assert_eq!(config.subprotocols.len(), 3);
        assert_eq!(config.subprotocols[0], "mqtt");
        assert_eq!(config.subprotocols[1], "chat-v2");
        assert_eq!(config.subprotocols[2], "chat-v1");
    }

    #[test]
    fn test_server_config_empty_subprotocols() {
        let config = ServerConfig::new("0.0.0.0:8080");

        assert_eq!(config.bind_address, "0.0.0.0:8080");
        assert!(config.subprotocols.is_empty());
    }

    #[test]
    fn test_client_config_with_header() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_header("Authorization", "Bearer token123")
            .with_header("X-API-Key", "secret-key");

        assert_eq!(config.headers.get("Authorization"), Some("Bearer token123"));
        assert_eq!(config.headers.get("X-API-Key"), Some("secret-key"));
    }

    #[test]
    fn test_client_config_with_headers() {
        use crate::headers::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer token123").unwrap();
        headers.insert("User-Agent", "TestClient").unwrap();

        let config = ClientConfig::new("ws://localhost:8080")
            .with_headers(&headers);

        assert_eq!(config.headers.get("Authorization"), Some("Bearer token123"));
        assert_eq!(config.headers.get("User-Agent"), Some("TestClient"));
    }

    #[test]
    fn test_client_config_with_authorization() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_authorization("Bearer token123");

        assert_eq!(config.headers.get_authorization(), Some("Bearer token123"));
    }

    #[test]
    fn test_client_config_with_user_agent() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_user_agent("MyClient/1.0");

        assert_eq!(config.headers.get_user_agent(), Some("MyClient/1.0"));
    }

    #[test]
    fn test_client_config_with_origin() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_origin("https://example.com");

        assert_eq!(config.headers.get_origin(), Some("https://example.com"));
    }

    #[test]
    fn test_client_config_with_cookie() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_cookie("session=abc123");

        assert_eq!(config.headers.get_cookie(), Some("session=abc123"));
    }

    #[test]
    fn test_client_config_with_api_key() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_api_key("secret-key-123");

        assert_eq!(config.headers.get_api_key(), Some("secret-key-123"));
    }

    #[test]
    fn test_server_config_with_header() {
        let config = ServerConfig::new("0.0.0.0:8080")
            .with_header("X-Server", "MyServer/1.0")
            .with_header("X-Custom", "value");

        assert_eq!(config.headers.get("X-Server"), Some("MyServer/1.0"));
        assert_eq!(config.headers.get("X-Custom"), Some("value"));
    }

    #[test]
    fn test_server_config_with_headers() {
        use crate::headers::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert("X-Server", "MyServer/1.0").unwrap();
        headers.insert("X-Custom", "value").unwrap();

        let config = ServerConfig::new("0.0.0.0:8080")
            .with_headers(&headers);

        assert_eq!(config.headers.get("X-Server"), Some("MyServer/1.0"));
        assert_eq!(config.headers.get("X-Custom"), Some("value"));
    }

    #[test]
    fn test_server_config_with_required_header() {
        let config = ServerConfig::new("0.0.0.0:8080")
            .with_required_header("X-API-Key")
            .with_required_header("Authorization");

        assert_eq!(config.required_headers.len(), 2);
        assert!(config.required_headers.contains(&"X-API-Key".to_string()));
        assert!(config.required_headers.contains(&"Authorization".to_string()));
    }

    #[test]
    fn test_client_config_multiple_headers() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_authorization("Bearer token123")
            .with_user_agent("MyClient/1.0")
            .with_origin("https://example.com")
            .with_api_key("secret-key");

        assert_eq!(config.headers.get_authorization(), Some("Bearer token123"));
        assert_eq!(config.headers.get_user_agent(), Some("MyClient/1.0"));
        assert_eq!(config.headers.get_origin(), Some("https://example.com"));
        assert_eq!(config.headers.get_api_key(), Some("secret-key"));
    }
}

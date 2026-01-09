//! Error types for websocket-fabric

use std::io;

/// Errors that can occur in WebSocket operations
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Connection was closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Reconnection timeout
    #[error("Reconnection timeout")]
    ReconnectTimeout,

    /// Invalid WebSocket frame
    #[error("Invalid frame: {0}")]
    InvalidFrame(String),

    /// Message too large
    #[error("Message too large: {size} bytes (max {max} bytes)")]
    MessageTooLarge { size: usize, max: usize },

    /// UTF-8 validation error
    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    /// Protocol violation
    #[error("Protocol violation: {0}")]
    ProtocolViolation(String),

    /// Handshake failed
    #[error("Handshake failed: {0}")]
    HandshakeFailed(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// WebSocket error
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// URL parse error
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// Buffer full
    #[error("Buffer full")]
    BufferFull,

    /// Backpressure timeout
    #[error("Backpressure timeout")]
    BackpressureTimeout,

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// TLS error
    #[error("TLS error: {0}")]
    Tls(String),

    /// Operation not supported
    #[error("Operation not supported: {0}")]
    NotSupported(String),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Timeout
    #[error("Operation timed out")]
    Timeout,
}

/// Result type for WebSocket operations
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a connection failed error
    pub fn connection_failed(msg: impl Into<String>) -> Self {
        Error::ConnectionFailed(msg.into())
    }

    /// Create an invalid frame error
    pub fn invalid_frame(msg: impl Into<String>) -> Self {
        Error::InvalidFrame(msg.into())
    }

    /// Create a protocol violation error
    pub fn protocol_violation(msg: impl Into<String>) -> Self {
        Error::ProtocolViolation(msg.into())
    }

    /// Create a handshake failed error
    pub fn handshake_failed(msg: impl Into<String>) -> Self {
        Error::HandshakeFailed(msg.into())
    }

    /// Create an invalid URL error
    pub fn invalid_url(msg: impl Into<String>) -> Self {
        Error::InvalidUrl(msg.into())
    }

    /// Create an authentication failed error
    pub fn authentication_failed(msg: impl Into<String>) -> Self {
        Error::AuthenticationFailed(msg.into())
    }

    /// Create a TLS error
    pub fn tls(msg: impl Into<String>) -> Self {
        Error::Tls(msg.into())
    }

    /// Create a not supported error
    pub fn not_supported(msg: impl Into<String>) -> Self {
        Error::NotSupported(msg.into())
    }

    /// Create an invalid state error
    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Error::InvalidState(msg.into())
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Io(_) |
                Error::ConnectionClosed |
                Error::Timeout |
                Error::Tls(_)
        )
    }

    /// Check if error should trigger reconnection
    pub fn should_reconnect(&self) -> bool {
        matches!(
            self,
            Error::Io(_) |
                Error::ConnectionClosed |
                Error::Timeout
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::connection_failed("test");
        assert!(err.to_string().contains("Connection failed"));

        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let err = Error::Io(io_err);
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn test_is_retryable() {
        let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "test");
        let err = Error::Io(io_err);
        assert!(err.is_retryable());

        let err = Error::invalid_frame("test");
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_should_reconnect() {
        let err = Error::ConnectionClosed;
        assert!(err.should_reconnect());

        let err = Error::InvalidFrame("test".to_string());
        assert!(!err.should_reconnect());
    }

    #[test]
    fn test_error_convenience_methods() {
        let err = Error::handshake_failed("invalid key");
        assert!(matches!(err, Error::HandshakeFailed(_)));

        let err = Error::invalid_url("missing scheme");
        assert!(matches!(err, Error::InvalidUrl(_)));

        let err = Error::authentication_failed("invalid token");
        assert!(matches!(err, Error::AuthenticationFailed(_)));
    }
}

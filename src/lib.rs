//! # websocket-fabric
//!
//! High-performance WebSocket library providing sub-millisecond latency real-time communication.
//!
//! ## Features
//!
//! - **Ultra-low latency**: P50 <100µs, P95 <500µs
//! - **High throughput**: >100K messages/sec per connection
//! - **Connection pooling**: Automatic connection reuse and management
//! - **Auto-reconnection**: Exponential backoff reconnection logic
//! - **Backpressure handling**: Prevents overwhelming servers/clients
//! - **Heartbeat monitoring**: Automatic ping/pong keepalive
//! - **Zero-copy**: Optimized message passing with Bytes
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use websocket_fabric::{WebSocketClient, Message};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to WebSocket server
//!     let mut client = WebSocketClient::connect("ws://echo.websocket.org").await?;
//!
//!     // Send text message
//!     client.send_text("Hello, WebSocket!").await?;
//!
//!     // Receive message
//!     if let Some(msg) = client.recv().await? {
//!         println!("Received: {:?}", msg);
//!     }
//!
//!     // Close connection
//!     client.close(None).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! The library is organized into several modules:
//!
//! - `client`: WebSocket client with connection management
//! - `message`: Message types and framing
//! - `error`: Error types
//! - `config`: Configuration structures
//! - `reconnect`: Reconnection logic
//! - `backpressure`: Backpressure control
//! - `heartbeat`: Ping/pong keepalive
//! - `metrics`: Performance metrics

pub mod client;
pub mod message;
pub mod error;
pub mod config;
pub mod reconnect;
pub mod backpressure;
pub mod heartbeat;
pub mod metrics;

// Re-export commonly used types
pub use client::WebSocketClient;
pub use message::{Message, MessageType, Frame};
pub use error::{Error, Result};
pub use config::{ClientConfig, ServerConfig, ReconnectConfig, BackpressureConfig, HeartbeatConfig};
pub use metrics::MetricsCollector;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default maximum message size (10MB)
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Default ping interval (30 seconds)
pub const DEFAULT_PING_INTERVAL: u64 = 30;

/// Default ping timeout (10 seconds)
pub const DEFAULT_PING_TIMEOUT: u64 = 10;

/// Default buffer size for message channels (1000 messages)
pub const DEFAULT_BUFFER_SIZE: usize = 1000;

/// Default connection timeout (5 seconds)
pub const DEFAULT_CONNECTION_TIMEOUT: u64 = 5;

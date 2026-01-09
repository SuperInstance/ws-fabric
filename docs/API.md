# websocket-fabric API Reference

This document provides a complete API reference for websocket-fabric.

## Table of Contents

1. [Core Types](#core-types)
2. [Client API](#client-api)
3. [Message API](#message-api)
4. [Configuration API](#configuration-api)
5. [Error Types](#error-types)
6. [Metrics API](#metrics-api)
7. [Modules](#modules)

---

## Core Types

### WebSocketClient

The main WebSocket client that manages connections and message handling.

**Definition**: `client::WebSocketClient`

#### Methods

##### `connect`

```rust
pub async fn connect(url: impl Into<String>) -> Result<Self>
```

Connect to a WebSocket server with default configuration.

**Parameters**:
- `url`: WebSocket URL (e.g., `ws://localhost:8080`)

**Returns**: `WebSocketClient` instance

**Errors**:
- `Error::ConnectionFailed`: Connection establishment failed
- `Error::InvalidUrl`: Invalid URL format
- `Error::Timeout`: Connection timeout

**Example**:
```rust
let client = WebSocketClient::connect("ws://localhost:8080").await?;
```

---

##### `connect_with_config`

```rust
pub async fn connect_with_config(config: ClientConfig) -> Result<Self>
```

Connect with custom configuration.

**Parameters**:
- `config`: Client configuration

**Returns**: `WebSocketClient` instance

**Example**:
```rust
let config = ClientConfig::new("ws://localhost:8080")
    .with_max_message_size(16 * 1024 * 1024);
let client = WebSocketClient::connect_with_config(config).await?;
```

---

##### `send_text`

```rust
pub async fn send_text(&self, text: &str) -> Result<()>
```

Send a text message.

**Parameters**:
- `text`: Text content to send

**Returns**: `Ok(())` on success

**Errors**:
- `Error::BufferFull`: Backpressure active
- `Error::ConnectionClosed`: Connection closed
- `Error::MessageTooLarge`: Message exceeds size limit

**Example**:
```rust
client.send_text("Hello, Server!").await?;
```

---

##### `send_binary`

```rust
pub async fn send_binary(&self, data: &[u8]) -> Result<()>
```

Send a binary message.

**Parameters**:
- `data`: Binary data to send

**Returns**: `Ok(())` on success

**Errors**: Same as `send_text`

**Example**:
```rust
client.send_binary(&[1, 2, 3, 4]).await?;
```

---

##### `send`

```rust
pub async fn send(&self, msg: Message) -> Result<()>
```

Send a message of any type.

**Parameters**:
- `msg`: Message to send

**Returns**: `Ok(())` on success

**Example**:
```rust
let msg = Message::ping(b"ping");
client.send(msg).await?;
```

---

##### `recv`

```rust
pub async fn recv(&self) -> Result<Option<Message>>
```

Receive a message (blocking).

**Returns**: `Some(Message)` if message available, `None` if closed

**Note**: This is a simplified implementation. Full receiver API planned for future release.

---

##### `is_connected`

```rust
pub fn is_connected(&self) -> bool
```

Check if client is connected.

**Returns**: `true` if connected, `false` otherwise

**Example**:
```rust
if client.is_connected() {
    println!("Connected!");
}
```

---

##### `close`

```rust
pub async fn close(&self, reason: Option<String>) -> Result<()>
```

Close the connection.

**Parameters**:
- `reason`: Optional close reason

**Returns**: `Ok(())` on success

**Example**:
```rust
client.close(Some("Shutdown requested".to_string())).await?;
```

---

##### `metrics`

```rust
pub fn metrics(&self) -> &MetricsCollector
```

Get metrics collector.

**Returns**: Reference to `MetricsCollector`

**Example**:
```rust
let metrics = client.metrics();
println!("Messages sent: {}", metrics.messages_sent());
```

---

##### `heartbeat`

```rust
pub fn heartbeat(&self) -> &HeartbeatManager
```

Get heartbeat manager.

**Returns**: Reference to `HeartbeatManager`

**Example**:
```rust
let heartbeat = client.heartbeat();
println!("Pings sent: {}", heartbeat.ping_count());
```

---

##### `backpressure`

```rust
pub fn backpressure(&self) -> &BackpressureController
```

Get backpressure controller.

**Returns**: Reference to `BackpressureController`

**Example**:
```rust
let backpressure = client.backpressure();
println!("Utilization: {:.1}%", backpressure.utilization() * 100.0);
```

---

### Message

WebSocket message with zero-copy payload.

**Definition**: `message::Message`

#### Constructor Methods

##### `text`

```rust
pub fn text(text: impl Into<String>) -> Self
```

Create a text message.

**Example**:
```rust
let msg = Message::text("Hello");
```

---

##### `binary`

```rust
pub fn binary(data: impl Into<Bytes>) -> Self
```

Create a binary message.

**Example**:
```rust
let msg = Message::binary(vec![1, 2, 3, 4]);
```

---

##### `binary_from_slice`

```rust
pub fn binary_from_slice(data: &[u8]) -> Self
```

Create a binary message from a slice (copies data).

**Example**:
```rust
let data = &[1, 2, 3, 4];
let msg = Message::binary_from_slice(data);
```

---

##### `ping`

```rust
pub fn ping(data: impl Into<Bytes>) -> Self
```

Create a ping message.

**Example**:
```rust
let msg = Message::ping(&b"ping"[..]);
```

---

##### `pong`

```rust
pub fn pong(data: impl Into<Bytes>) -> Self
```

Create a pong message.

**Example**:
```rust
let msg = Message::pong(&b"pong"[..]);
```

---

##### `close`

```rust
pub fn close(code: Option<u16>, reason: Option<String>) -> Self
```

Create a close message.

**Parameters**:
- `code`: WebSocket close code (e.g., 1000 for normal closure)
- `reason`: Close reason string

**Example**:
```rust
let msg = Message::close(Some(1000), Some("Goodbye".to_string()));
```

---

##### `json`

```rust
pub fn json<T: serde::Serialize>(value: &T) -> Result<Self>
```

Create a JSON message from serializable type.

**Example**:
```rust
#[derive(Serialize)]
struct Data {
    value: i32,
}

let data = Data { value: 42 };
let msg = Message::json(&data)?;
```

---

#### Inspection Methods

##### `as_text`

```rust
pub fn as_text(&self) -> Result<String>
```

Get message as text string.

**Returns**: Text content

**Errors**:
- `Error::InvalidFrame`: Not a text message
- `Error::Utf8Error`: Invalid UTF-8

---

##### `as_bytes`

```rust
pub fn as_bytes(&self) -> &[u8]
```

Get message payload as bytes.

---

##### `payload`

```rust
pub fn payload(&self) -> &Bytes
```

Get message payload as Bytes reference.

---

##### `msg_type`

```rust
pub fn msg_type(&self) -> &MessageType
```

Get message type.

---

##### `is_text`, `is_binary`, `is_ping`, `is_pong`, `is_close`

```rust
pub fn is_text(&self) -> bool
pub fn is_binary(&self) -> bool
pub fn is_ping(&self) -> bool
pub fn is_pong(&self) -> bool
pub fn is_close(&self) -> bool
```

Check message type.

---

##### `parse_json`

```rust
pub fn parse_json<T: serde::de::DeserializeOwned>(&self) -> Result<T>
```

Parse message as JSON.

**Example**:
```rust
#[derive(Deserialize)]
struct Data {
    value: i32,
}

let data: Data = message.parse_json()?;
```

---

##### `len`, `is_empty`

```rust
pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
```

Get message size in bytes.

---

### MessageType

WebSocket message type enum.

**Definition**: `message::MessageType`

#### Variants

```rust
pub enum MessageType {
    Text,         // UTF-8 text message
    Binary,       // Binary message
    Ping,         // Ping control frame
    Pong,         // Pong control frame
    Close,        // Close frame
    Continuation, // Continuation frame
}
```

---

### Error

Error type for WebSocket operations.

**Definition**: `error::Error`

#### Variants

```rust
pub enum Error {
    ConnectionFailed(String),
    ConnectionClosed,
    ReconnectTimeout,
    InvalidFrame(String),
    MessageTooLarge { size: usize, max: usize },
    Utf8Error(String),
    ProtocolViolation(String),
    HandshakeFailed(String),
    Io(std::io::Error),
    WebSocket(String),
    InvalidUrl(String),
    BufferFull,
    BackpressureTimeout,
    AuthenticationFailed(String),
    Tls(String),
    NotSupported(String),
    InvalidState(String),
    Timeout,
}
```

#### Methods

##### `is_retryable`

```rust
pub fn is_retryable(&self) -> bool
```

Check if error is retryable.

**Returns**: `true` if operation can be retried

---

##### `should_reconnect`

```rust
pub fn should_reconnect(&self) -> bool
```

Check if reconnection should occur.

**Returns**: `true` if reconnection recommended

---

### Result

Type alias for Result with Error.

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

---

## Configuration API

### ClientConfig

Client configuration with builder pattern.

**Definition**: `config::ClientConfig`

#### Constructor

##### `new`

```rust
pub fn new(url: impl Into<String>) -> Self
```

Create default client configuration.

---

#### Builder Methods

##### `with_max_message_size`

```rust
pub fn with_max_message_size(mut self, size: usize) -> Self
```

Set maximum message size in bytes.

**Default**: 10MB

---

##### `with_auto_reconnect`

```rust
pub fn with_auto_reconnect(mut self, enabled: bool) -> Self
```

Enable/disable auto-reconnect.

**Default**: `true`

---

##### `with_reconnect_config`

```rust
pub fn with_reconnect_config(mut self, config: ReconnectConfig) -> Self
```

Set reconnection configuration.

---

##### `with_heartbeat_config`

```rust
pub fn with_heartbeat_config(mut self, config: HeartbeatConfig) -> Self
```

Set heartbeat configuration.

---

##### `with_backpressure_config`

```rust
pub fn with_backpressure_config(mut self, config: BackpressureConfig) -> Self
```

Set backpressure configuration.

---

##### `with_connection_timeout`

```rust
pub fn with_connection_timeout(mut self, timeout: Duration) -> Self
```

Set connection timeout.

**Default**: 5 seconds

---

### ReconnectConfig

Reconnection configuration.

**Definition**: `config::ReconnectConfig`

#### Constructor

##### `new` / `default`

```rust
pub fn new() -> Self
pub fn default() -> Self
```

Create default reconnection configuration.

**Defaults**:
- `enabled`: true
- `max_attempts`: 0 (infinite)
- `initial_delay`: 100ms
- `max_delay`: 30s
- `backoff_multiplier`: 1.5

---

##### `disabled`

```rust
pub fn disabled() -> Self
```

Create configuration with reconnection disabled.

---

#### Builder Methods

##### `with_max_attempts`

```rust
pub fn with_max_attempts(mut self, max: u32) -> Self
```

Set maximum reconnection attempts (0 = infinite).

---

##### `with_initial_delay`

```rust
pub fn with_initial_delay(mut self, delay: Duration) -> Self
```

Set initial reconnection delay.

---

##### `with_max_delay`

```rust
pub fn with_max_delay(mut self, delay: Duration) -> Self
```

Set maximum delay between attempts.

---

##### `with_backoff_multiplier`

```rust
pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self
```

Set exponential backoff multiplier.

---

### HeartbeatConfig

Heartbeat/ping-pong configuration.

**Definition**: `config::HeartbeatConfig`

#### Constructor

##### `new` / `default`

```rust
pub fn new() -> Self
pub fn default() -> Self
```

Create default heartbeat configuration.

**Defaults**:
- `enabled`: true
- `ping_interval`: 30s
- `ping_timeout`: 10s

---

##### `disabled`

```rust
pub fn disabled() -> Self
```

Create configuration with heartbeat disabled.

---

#### Builder Methods

##### `with_ping_interval`

```rust
pub fn with_ping_interval(mut self, interval: Duration) -> Self
```

Set time between pings.

---

##### `with_ping_timeout`

```rust
pub fn with_ping_timeout(mut self, timeout: Duration) -> Self
```

Set time to wait for pong.

---

### BackpressureConfig

Backpressure configuration.

**Definition**: `config::BackpressureConfig`

#### Constructor

##### `new` / `default`

```rust
pub fn new() -> Self
pub fn default() -> Self
```

Create default backpressure configuration.

**Defaults**:
- `enabled`: true
- `max_buffer_size`: 1000 messages
- `backpressure_threshold`: 0.8 (80%)
- `recovery_threshold`: 0.6 (60%)

---

##### `disabled`

```rust
pub fn disabled() -> Self
```

Create configuration with backpressure disabled.

---

#### Builder Methods

##### `with_max_buffer_size`

```rust
pub fn with_max_buffer_size(mut self, size: usize) -> Self
```

Set maximum buffer size in messages.

---

##### `with_backpressure_threshold`

```rust
pub fn with_backpressure_threshold(mut self, threshold: f64) -> Self
```

Set backpressure activation threshold (0.0-1.0).

---

##### `with_recovery_threshold`

```rust
pub fn with_recovery_threshold(mut self, threshold: f64) -> Self
```

Set recovery threshold (0.0-1.0).

---

## Metrics API

### MetricsCollector

Collects and reports connection metrics.

**Definition**: `metrics::MetricsCollector`

#### Methods

##### `new`

```rust
pub fn new() -> Self
```

Create new metrics collector.

---

##### `record_connection`

```rust
pub fn record_connection(&self)
```

Record a new connection.

---

##### `record_disconnection`

```rust
pub fn record_disconnection(&self)
```

Record a disconnection.

---

##### `record_message_sent`

```rust
pub fn record_message_sent(&self, size: usize)
```

Record a sent message.

---

##### `record_message_received`

```rust
pub fn record_message_received(&self, size: usize)
```

Record a received message.

---

##### `record_send_error`

```rust
pub fn record_send_error(&self)
```

Record a send error.

---

##### `record_receive_error`

```rust
pub fn record_receive_error(&self)
```

Record a receive error.

---

##### `record_latency`

```rust
pub fn record_latency(&self, latency: Duration)
```

Record operation latency.

---

##### `total_connections`

```rust
pub fn total_connections(&self) -> u64
```

Get total connection count.

---

##### `active_connections`

```rust
pub fn active_connections(&self) -> u64
```

Get active connection count.

---

##### `messages_sent`

```rust
pub fn messages_sent(&self) -> u64
```

Get message sent count.

---

##### `messages_received`

```rust
pub fn messages_received(&self) -> u64
```

Get message received count.

---

##### `bytes_sent`

```rust
pub fn bytes_sent(&self) -> u64
```

Get total bytes sent.

---

##### `bytes_received`

```rust
pub fn bytes_received(&self) -> u64
```

Get total bytes received.

---

##### `send_errors`

```rust
pub fn send_errors(&self) -> u64
```

Get send error count.

---

##### `receive_errors`

```rust
pub fn receive_errors(&self) -> u64
```

Get receive error count.

---

##### `latency_percentiles`

```rust
pub fn latency_percentiles(&self) -> LatencyPercentiles
```

Get latency percentiles.

---

##### `idle_time`

```rust
pub fn idle_time(&self) -> Option<Duration>
```

Get time since last activity.

---

##### `snapshot`

```rust
pub fn snapshot(&self) -> MetricsSnapshot
```

Get metrics snapshot.

---

##### `reset`

```rust
pub fn reset(&self)
```

Reset all metrics.

---

### LatencyPercentiles

Latency percentile values.

**Definition**: `metrics::LatencyPercentiles`

```rust
pub struct LatencyPercentiles {
    pub p50: u64,   // 50th percentile
    pub p95: u64,   // 95th percentile
    pub p99: u64,   // 99th percentile
    pub p999: u64,  // 99.9th percentile
}
```

All values are in microseconds (µs).

---

### MetricsSnapshot

Snapshot of metrics at a point in time.

**Definition**: `metrics::MetricsSnapshot`

```rust
pub struct MetricsSnapshot {
    pub connections_created: u64,
    pub connections_closed: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub send_errors: u64,
    pub receive_errors: u64,
}
```

---

## Modules

### Module List

- `client`: WebSocket client implementation
- `message`: Message types and framing
- `error`: Error types
- `config`: Configuration builders
- `reconnect`: Reconnection logic
- `heartbeat`: Ping/pong keepalive
- `backpressure`: Flow control
- `metrics`: Metrics collection

### Re-exports

The library re-exports common types:

```rust
pub use client::WebSocketClient;
pub use message::{Message, MessageType, Frame};
pub use error::{Error, Result};
pub use config::{ClientConfig, ServerConfig, ReconnectConfig, BackpressureConfig, HeartbeatConfig};
pub use metrics::MetricsCollector;
```

---

## Constants

```rust
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
```

---

For more information, see:
- [USER_GUIDE.md](USER_GUIDE.md) - Usage examples
- [DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md) - Contributing guide
- [ARCHITECTURE.md](ARCHITECTURE.md) - Architecture details

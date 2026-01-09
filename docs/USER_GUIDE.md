# websocket-fabric User Guide

This guide provides comprehensive information on using websocket-fabric in your applications.

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Basic Usage](#basic-usage)
4. [Advanced Configuration](#advanced-configuration)
5. [Message Handling](#message-handling)
6. [Error Handling](#error-handling)
7. [Metrics and Monitoring](#metrics-and-monitoring)
8. [Best Practices](#best-practices)
9. [Troubleshooting](#troubleshooting)
10. [FAQ](#faq)

## Installation

Add websocket-fabric to your `Cargo.toml`:

```toml
[dependencies]
websocket-fabric = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Features

The library uses Tokio for async runtime. Ensure you have the `full` feature set enabled:

```toml
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### Minimal Example

```rust
use websocket_fabric::WebSocketClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to a WebSocket server
    let client = WebSocketClient::connect("ws://localhost:8080").await?;

    // Send a message
    client.send_text("Hello, World!").await?;

    // Close the connection
    client.close(None).await?;

    Ok(())
}
```

## Basic Usage

### Connecting to a Server

```rust
use websocket_fabric::WebSocketClient;

// Simple connection
let client = WebSocketClient::connect("ws://echo.websocket.org").await?;

// Check connection status
if client.is_connected() {
    println!("Connected!");
}
```

### Sending Messages

#### Text Messages

```rust
// Simple text
client.send_text("Hello, Server!").await?;

// Formatted text
let message = format!("User {} joined", username);
client.send_text(&message).await?;
```

#### Binary Messages

```rust
// From slice
let data = vec![0x01, 0x02, 0x03, 0x04];
client.send_binary(&data).await?;

// Direct bytes
client.send_binary(&[1, 2, 3, 4]).await?;
```

#### JSON Messages

```rust
use serde::{Serialize, Deserialize};
use websocket_fabric::Message;

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    from: String,
    to: String,
    text: String,
}

let msg = ChatMessage {
    from: "alice".to_string(),
    to: "bob".to_string(),
    text: "Hello!".to_string(),
};

let ws_msg = Message::json(&msg)?;
client.send(ws_msg).await?;
```

### Receiving Messages

The client handles incoming messages in background tasks. For custom message handling, you'll need to implement your own receiver channel:

```rust
use websocket_fabric::{WebSocketClient, ClientConfig, Message};
use tokio::sync::mpsc;

// Create custom configuration with receiver channel
let config = ClientConfig::new("ws://localhost:8080");

// Note: In a real application, you would set up a receiver channel
// to handle incoming messages. See examples/ for complete implementations.
```

### Closing Connections

```rust
// Close without reason
client.close(None).await?;

// Close with reason
client.close(Some("Shutdown requested".to_string())).await?;

// Close with status code and reason
use websocket_fabric::Message;
let close_msg = Message::close(Some(1000), Some("Normal closure".to_string()));
client.send(close_msg).await?;
```

## Advanced Configuration

### Client Configuration

```rust
use websocket_fabric::{ClientConfig, WebSocketClient};
use std::time::Duration;

let config = ClientConfig::new("ws://localhost:8080")
    .with_max_message_size(16 * 1024 * 1024)  // 16MB max message
    .with_auto_reconnect(true)                  // Enable auto-reconnect
    .with_connection_timeout(Duration::from_secs(10));  // 10s timeout

let client = WebSocketClient::connect_with_config(config).await?;
```

### Reconnection Configuration

Configure automatic reconnection with exponential backoff:

```rust
use websocket_fabric::{ClientConfig, ReconnectConfig};
use std::time::Duration;

// Custom reconnection settings
let reconnect_config = ReconnectConfig::new()
    .with_max_attempts(10)                      // Max 10 attempts
    .with_initial_delay(Duration::from_millis(100))  // Start at 100ms
    .with_max_delay(Duration::from_secs(30))    // Cap at 30s
    .with_backoff_multiplier(2.0);              // Double each time

let config = ClientConfig::new("ws://localhost:8080")
    .with_reconnect_config(reconnect_config);

// Disable auto-reconnect
let config = ClientConfig::new("ws://localhost:8080")
    .with_reconnect_config(ReconnectConfig::disabled());
```

**Reconnection Behavior**:

1. Connection lost → Wait `initial_delay`
2. Reconnect attempt #1 fails → Wait `initial_delay * multiplier`
3. Reconnect attempt #2 fails → Wait `previous_delay * multiplier`
4. Continue until `max_delay` or `max_attempts` reached

### Heartbeat Configuration

Keep connections alive with ping/pong:

```rust
use websocket_fabric::{ClientConfig, HeartbeatConfig};
use std::time::Duration;

// Custom heartbeat settings
let heartbeat_config = HeartbeatConfig::new()
    .with_ping_interval(Duration::from_secs(20))  // Ping every 20s
    .with_ping_timeout(Duration::from_secs(10));  // Expect pong within 10s

let config = ClientConfig::new("ws://localhost:8080")
    .with_heartbeat_config(heartbeat_config);

// Disable heartbeat
let config = ClientConfig::new("ws://localhost:8080")
    .with_heartbeat_config(HeartbeatConfig::disabled());
```

**How Heartbeat Works**:

1. Client sends ping message every `ping_interval`
2. Waits for pong response within `ping_timeout`
3. If 4 consecutive pings timeout, connection considered dead
4. Automatic reconnection triggered (if enabled)

### Backpressure Configuration

Prevent buffer overflow with backpressure control:

```rust
use websocket_fabric::{ClientConfig, BackpressureConfig};

// Custom backpressure settings
let backpressure_config = BackpressureConfig::new()
    .with_max_buffer_size(5000)           // 5000 messages max
    .with_backpressure_threshold(0.75)    // Activate at 75% full
    .with_recovery_threshold(0.50);       // Recover at 50% full

let config = ClientConfig::new("ws://localhost:8080")
    .with_backpressure_config(backpressure_config);

// Disable backpressure
let config = ClientConfig::new("ws://localhost:8080")
    .with_backpressure_config(BackpressureConfig::disabled());
```

**How Backpressure Works**:

1. Monitor buffer utilization
2. When buffer > `backpressure_threshold` (e.g., 80%):
   - `send()` returns `Error::BufferFull`
   - Application should throttle sends
3. When buffer < `recovery_threshold` (e.g., 60%):
   - Backpressure deactivated
   - Normal sending resumes

## Message Handling

### Message Types

websocket-fabric supports all WebSocket message types:

```rust
use websocket_fabric::Message;

// Text message
let text_msg = Message::text("Hello");

// Binary message
let bin_msg = Message::binary(vec![1, 2, 3, 4]);

// Ping message
let ping_msg = Message::ping(b"ping");

// Pong message
let pong_msg = Message::pong(b"pong");

// Close message
let close_msg = Message::close(Some(1000), Some("Goodbye".to_string()));
```

### Message Inspection

```rust
use websocket_fabric::{Message, MessageType};

fn handle_message(msg: Message) {
    match msg.msg_type() {
        MessageType::Text => {
            let text = msg.as_text().unwrap();
            println!("Received text: {}", text);
        }
        MessageType::Binary => {
            let data = msg.as_bytes();
            println!("Received {} bytes", data.len());
        }
        MessageType::Ping => {
            println!("Received ping");
            // Pong is sent automatically
        }
        MessageType::Pong => {
            println!("Received pong");
        }
        MessageType::Close => {
            println!("Received close frame");
        }
        MessageType::Continuation => {
            println!("Received continuation frame");
        }
    }
}
```

### JSON Serialization

**Sending JSON**:

```rust
use serde::Serialize;
use websocket_fabric::Message;

#[derive(Serialize)]
struct Update {
    id: u64,
    status: String,
    progress: f64,
}

let update = Update {
    id: 123,
    status: "processing".to_string(),
    progress: 0.75,
};

let msg = Message::json(&update)?;
client.send(msg).await?;
```

**Receiving JSON**:

```rust
use serde::Deserialize;
use websocket_fabric::Message;

#[derive(Deserialize)]
struct Update {
    id: u64,
    status: String,
    progress: f64,
}

// Assuming you have a message
let update: Update = message.parse_json()?;
println!("Update {}: {} = {}", update.id, update.status, update.progress);
```

## Error Handling

### Error Types

```rust
use websocket_fabric::Error;

match client.send_text("test").await {
    Ok(()) => println!("Sent successfully"),
    Err(Error::ConnectionFailed(msg)) => {
        eprintln!("Connection failed: {}", msg);
    }
    Err(Error::ConnectionClosed) => {
        eprintln!("Connection was closed");
    }
    Err(Error::BufferFull) => {
        eprintln!("Buffer full, backing off");
        // Wait and retry
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err(Error::Timeout) => {
        eprintln!("Operation timed out");
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

### Retryable Errors

Check if an error is retryable:

```rust
use websocket_fabric::Error;

async fn send_with_retry(client: &WebSocketClient, text: &str, max_retries: u32) -> Result<(), Error> {
    let mut retries = 0;

    loop {
        match client.send_text(text).await {
            Ok(()) => return Ok(()),
            Err(e) if e.is_retryable() && retries < max_retries => {
                retries += 1;
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Reconnection Decisions

Check if reconnection should occur:

```rust
use websocket_fabric::Error;

let should_reconnect = match error {
    Error::ConnectionClosed => true,
    Error::Io(_) => true,
    Error::Timeout => true,
    _ => false,
};
```

## Metrics and Monitoring

### Connection Metrics

```rust
use websocket_fabric::MetricsCollector;

let metrics = client.metrics();

// Connection stats
println!("Total connections: {}", metrics.total_connections());
println!("Active connections: {}", metrics.active_connections());
println!("Disconnections: {}", metrics.disconnections());

// Message stats
println!("Messages sent: {}", metrics.messages_sent());
println!("Messages received: {}", metrics.messages_received());
println!("Bytes sent: {}", metrics.bytes_sent());
println!("Bytes received: {}", metrics.bytes_received());

// Error stats
println!("Send errors: {}", metrics.send_errors());
println!("Receive errors: {}", metrics.receive_errors());
```

### Latency Metrics

```rust
let percentiles = metrics.latency_percentiles();

println!("Latency percentiles:");
println!("  P50: {}µs", percentiles.p50);
println!("  P95: {}µs", percentiles.p95);
println!("  P99: {}µs", percentiles.p99);
println!("  P99.9: {}µs", percentiles.p999);
```

### Activity Monitoring

```rust
// Check if connection is idle
if let Some(idle_time) = metrics.idle_time() {
    if idle_time > std::time::Duration::from_secs(60) {
        println!("Connection idle for {:?}", idle_time);
    }
}
```

### Metrics Snapshot

```rust
let snapshot = metrics.snapshot();
println!("Metrics snapshot:");
println!("  Messages sent: {}", snapshot.messages_sent);
println!("  Messages received: {}", snapshot.messages_received);
println!("  Bytes sent: {}", snapshot.bytes_sent);
println!("  Bytes received: {}", snapshot.bytes_received);
```

## Best Practices

### 1. Connection Lifecycle Management

```rust
// GOOD: Explicit connection lifecycle
let client = WebSocketClient::connect("ws://server").await?;

// Use client
client.send_text("Hello").await?;

// Explicit close
client.close(Some("Done".to_string())).await?;

// BAD: Let connection drop without close
let client = WebSocketClient::connect("ws://server").await?;
client.send_text("Hello").await?;
// Connection drops here without proper close
```

### 2. Error Handling

```rust
// GOOD: Handle specific errors
match client.send_text(data).await {
    Ok(()) => {},
    Err(Error::BufferFull) => {
        // Back off and retry
        tokio::time::sleep(Duration::from_millis(100)).await;
    },
    Err(e) => {
        eprintln!("Send error: {}", e);
        return Err(e.into());
    }
}

// BAD: Ignore all errors
let _ = client.send_text(data).await;
```

### 3. Backpressure Handling

```rust
// GOOD: Respect backpressure
loop {
    match client.send_text(message).await {
        Ok(()) => break,
        Err(Error::BufferFull) => {
            // Wait for buffer to drain
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }
        Err(e) => return Err(e.into()),
    }
}

// BAD: Spin on buffer full
loop {
    if client.send_text(message).await.is_ok() {
        break;
    }
    // CPU spins at 100%!
}
```

### 4. Resource Cleanup

```rust
// GOOD: Use scopes for cleanup
{
    let client = WebSocketClient::connect("ws://server").await?;
    // Use client...
    client.close(None).await?;
} // Client dropped here

// GOOD: Explicit cleanup in Drop
struct MyStruct {
    client: Option<WebSocketClient>,
}

impl MyStruct {
    async fn cleanup(&mut self) {
        if let Some(client) = self.client.take() {
            let _ = client.close(None).await;
        }
    }
}
```

### 5. Configuration Tuning

```rust
// GOOD: Tune for your workload
let config = ClientConfig::new("ws://server")
    // High-frequency trading: low latency, small buffers
    .with_max_message_size(1024)
    .with_heartbeat_config(HeartbeatConfig::new()
        .with_ping_interval(Duration::from_secs(5)))

    // Chat application: larger messages, larger buffers
    .with_max_message_size(10 * 1024 * 1024)
    .with_backpressure_config(BackpressureConfig::new()
        .with_max_buffer_size(10000));
```

## Troubleshooting

### Connection Issues

**Problem**: Can't connect to server

**Solutions**:
1. Check server URL format: `ws://` or `wss://`
2. Verify server is running: `curl http://localhost:8080`
3. Check firewall settings
4. Increase timeout: `.with_connection_timeout(Duration::from_secs(30))`

### High Latency

**Problem**: Messages take too long to send/receive

**Solutions**:
1. Check network latency: `ping server.com`
2. Disable Nagle's algorithm (if supported)
3. Reduce message size
4. Check backpressure: `client.backpressure().utilization()`
5. Monitor metrics: `client.metrics().latency_percentiles()`

### Memory Growth

**Problem**: Memory usage increasing over time

**Solutions**:
1. Reduce buffer size: `.with_max_buffer_size(500)`
2. Enable backpressure: `BackpressureConfig::new()`
3. Check for message leaks: ensure messages are consumed
4. Monitor metrics: `client.metrics().messages_sent()` vs `received()`

### Frequent Disconnections

**Problem**: Connection drops frequently

**Solutions**:
1. Check heartbeat settings: increase `ping_interval`
2. Check network stability
3. Enable auto-reconnect: `ReconnectConfig::new()`
4. Check server timeout settings

## FAQ

### Q: How do I handle concurrent sends?

A: The client is thread-safe and can be cloned:

```rust
let client1 = client.clone();
let client2 = client.clone();

tokio::spawn(async move {
    client1.send_text("From task 1").await.unwrap();
});

tokio::spawn(async move {
    client2.send_text("From task 2").await.unwrap();
});
```

### Q: Can I use TLS/SSL?

A: Yes, use `wss://` URL scheme:

```rust
let client = WebSocketClient::connect("wss://secure.example.com").await?;
```

### Q: How do I set custom headers?

A: Custom headers require modifying the underlying connection. This feature will be added in a future release.

### Q: What's the maximum message size?

A: Default is 10MB. Configure with:

```rust
let config = ClientConfig::new("ws://server")
    .with_max_message_size(100 * 1024 * 1024); // 100MB
```

### Q: How do I implement a message handler?

A: Currently, the client handles messages internally. Future versions will expose a receiver API for custom message handling.

### Q: Can I use this in a blocking context?

A: The library is async-only. Use `tokio::runtime::Runtime` in blocking code:

```rust
let rt = tokio::runtime::Runtime::new()?;
let client = rt.block_on(async {
    WebSocketClient::connect("ws://server").await?
});
```

For more information, see [API.md](API.md) and [DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md).

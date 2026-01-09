# websocket-fabric

> High-performance WebSocket library with sub-millisecond latency for Rust

## Overview

**websocket-fabric** is a production-ready WebSocket library built on tokio-tungstenite, designed for applications requiring ultra-low latency and high throughput. Perfect for real-time systems, game servers, financial trading platforms, and chat applications.

### Key Features

- **Ultra-low latency**: Zero-copy message passing with `bytes::Bytes`
- **High performance**: >100K messages/sec per connection
- **Resilient**: Automatic reconnection with exponential backoff
- **Reliable**: Heartbeat/ping-pong keepalive mechanism
- **Smart**: Backpressure control to prevent buffer overflow
- **Observable**: Comprehensive metrics with latency percentiles
- **Type-safe**: Full Rust type safety with serde JSON support
- **Async**: Built on Tokio for efficient async I/O

## Performance

- **Latency**: P50 <100µs, P95 <500µs
- **Throughput**: >100K messages/sec
- **Memory**: <10KB per connection
- **Scalability**: >10K concurrent connections

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
websocket-fabric = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Basic Client

```rust
use websocket_fabric::WebSocketClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to WebSocket server
    let client = WebSocketClient::connect("ws://localhost:8080").await?;

    // Send text message
    client.send_text("Hello, WebSocket!").await?;

    // Send binary message
    client.send_binary(&[1, 2, 3, 4]).await?;

    // Close connection
    client.close(Some("Goodbye!".to_string())).await?;

    Ok(())
}
```

### Advanced Configuration

```rust
use websocket_fabric::{WebSocketClient, ClientConfig, ReconnectConfig, HeartbeatConfig};
use std::time::Duration;

let config = ClientConfig::new("ws://localhost:8080")
    .with_max_message_size(16 * 1024 * 1024)  // 16MB
    .with_reconnect_config(
        ReconnectConfig::new()
            .with_max_attempts(10)
            .with_initial_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(30))
            .with_backoff_multiplier(2.0)
    )
    .with_heartbeat_config(
        HeartbeatConfig::new()
            .with_ping_interval(Duration::from_secs(20))
            .with_ping_timeout(Duration::from_secs(10))
    );

let client = WebSocketClient::connect_with_config(config).await?;
```

### JSON Messages

```rust
use websocket_fabric::Message;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    username: String,
    content: String,
    timestamp: u64,
}

let msg = ChatMessage {
    username: "alice".to_string(),
    content: "Hello!".to_string(),
    timestamp: 1640000000,
};

// Send JSON
let ws_msg = Message::json(&msg)?;
client.send(ws_msg).await?;

// Receive JSON (in your message handler)
let received_msg = message.parse_json::<ChatMessage>()?;
```

## Configuration

### Client Options

- `max_message_size`: Maximum message size in bytes (default: 10MB)
- `auto_reconnect`: Automatically reconnect on connection loss (default: true)
- `connection_timeout`: Timeout for initial connection (default: 5s)

### Reconnection Options

- `enabled`: Enable automatic reconnection (default: true)
- `max_attempts`: Maximum reconnection attempts, 0 = infinite (default: 0)
- `initial_delay`: Initial delay before first reconnection (default: 100ms)
- `max_delay`: Maximum delay between attempts (default: 30s)
- `backoff_multiplier`: Exponential backoff multiplier (default: 1.5)

### Heartbeat Options

- `enabled`: Enable ping/pong keepalive (default: true)
- `ping_interval`: Time between pings (default: 30s)
- `ping_timeout`: Time to wait for pong (default: 10s)

### Backpressure Options

- `enabled`: Enable backpressure control (default: true)
- `max_buffer_size`: Maximum buffer size in messages (default: 1000)
- `backpressure_threshold`: Activation threshold 0.0-1.0 (default: 0.8)
- `recovery_threshold`: Recovery threshold 0.0-1.0 (default: 0.6)

## Metrics

Monitor your WebSocket connections:

```rust
use websocket_fabric::MetricsCollector;

let metrics = client.metrics();

// Get connection stats
println!("Connections: {}", metrics.total_connections());
println!("Messages sent: {}", metrics.messages_sent());
println!("Messages received: {}", metrics.messages_received());

// Get latency percentiles
let percentiles = metrics.latency_percentiles();
println!("P50: {}µs", percentiles.p50);
println!("P95: {}µs", percentiles.p95);
println!("P99: {}µs", percentiles.p99);
println!("P99.9: {}µs", percentiles.p999);

// Get error rates
println!("Send errors: {}", metrics.send_errors());
println!("Receive errors: {}", metrics.receive_errors());
```

## Error Handling

All operations return `Result<T, Error>`:

```rust
use websocket_fabric::Error;

match client.send_text("Hello").await {
    Ok(()) => println!("Message sent"),
    Err(Error::BufferFull) => eprintln!("Backpressure active, retry later"),
    Err(Error::ConnectionClosed) => eprintln!("Connection closed"),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Error Types

- `ConnectionFailed`: Connection establishment failed
- `ConnectionClosed`: Connection was closed
- `ReconnectTimeout`: Reconnection attempts exhausted
- `InvalidFrame`: Invalid WebSocket frame received
- `MessageTooLarge`: Message exceeds size limit
- `Utf8Error`: Text message not valid UTF-8
- `BufferFull`: Backpressure limit reached
- `Timeout`: Operation timed out
- `WebSocket`: Generic WebSocket protocol error

## Testing

Run the test suite:

```bash
cargo test
```

Run with output:

```bash
cargo test -- --nocapture
```

## Documentation

- [ARCHITECTURE.md](docs/ARCHITECTURE.md) - System architecture and design
- [USER_GUIDE.md](docs/USER_GUIDE.md) - Detailed usage guide
- [DEVELOPER_GUIDE.md](docs/DEVELOPER_GUIDE.md) - Contributor guide
- [API.md](docs/API.md) - API reference

## Examples

See the [examples/](examples/) directory for complete examples:

- `basic_client.rs` - Simple echo client
- `chat_client.rs` - Chat room client
- `metrics_monitor.rs` - Metrics monitoring example

## Benchmarks

Run benchmarks:

```bash
cargo bench
```

## Contributing

Contributions are welcome! Please see [DEVELOPER_GUIDE.md](docs/DEVELOPER_GUIDE.md) for details.

## License

MIT OR Apache-2.0

## Acknowledgments

Built with:
- [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite) - Async WebSocket library
- [tokio](https://tokio.rs/) - Async runtime
- [bytes](https://github.com/tokio-rs/bytes) - Zero-copy byte management
- [thiserror](https://github.com/dtolnay/thiserror) - Error handling

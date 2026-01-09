# websocket-fabric Architecture

## Overview

**websocket-fabric** is a high-performance WebSocket library providing sub-millisecond latency real-time communication for the equilibrium-tokens system and other applications. The library is implemented in Rust with Go bindings for cross-language compatibility.

## Design Goals

1. **Ultra-low latency**: P50 <100µs, P95 <500µs
2. **High throughput**: >100K messages/sec per connection
3. **Scalability**: >10K concurrent connections per server
4. **Efficiency**: <10KB memory per connection
5. **Reliability**: Graceful handling of backpressure and failures
6. **Integration**: Seamless integration with equilibrium-tokens

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                         Application                         │
│                  (equilibrium-tokens, etc.)                 │
└────────────────────┬────────────────────────────────────────┘
                     │
                     │ WebSocket API
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                      websocket-fabric                        │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐   │
│  │   Connection  │  │   Message     │  │    Stream     │   │
│  │   Manager     │  │   Framing     │  │  Processing   │   │
│  └───────────────┘  └───────────────┘  └───────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐   │
│  │  Compression  │  │ Backpressure  │  │  Reconnect    │   │
│  │   (DEFLATE)   │  │   Control     │  │    Logic      │   │
│  └───────────────┘  └───────────────┘  └───────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐   │
│  │   Transport   │  │    Security   │  │   Metrics     │   │
│  │   (TCP/SSL)   │  │  (TLS/Auth)   │  │  & Telemetry  │   │
│  └───────────────┘  └───────────────┘  └───────────────┘   │
└────────────────────┬────────────────────────────────────────┘
                     │
                     │ Network
                     ▼
              ┌──────────────┐
              │   TCP/IP     │
              │   Network    │
              └──────────────┘
```

## Core Components

### 1. Connection Manager

**Purpose**: Manage WebSocket connection lifecycle

**Responsibilities**:
- Connection establishment (handshake)
- Connection pooling and reuse
- Health monitoring (ping/pong)
- Graceful shutdown
- Automatic reconnection

**API**:
```rust
pub struct WebSocketClient {
    // Connection state
    state: Arc<AtomicU8>,
    // Message queue
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
}

impl WebSocketClient {
    pub async fn connect(url: &str) -> Result<Self>;
    pub async fn send_text(&mut self, text: &str) -> Result<()>;
    pub async fn send_binary(&mut self, data: &[u8]) -> Result<()>;
    pub async fn recv(&mut self) -> Result<Message>;
    pub fn is_connected(&self) -> bool;
    pub async fn close(&mut self, frame: CloseFrame) -> Result<()>;
}
```

**Implementation Details**:
- Uses Tokio for async I/O
- Connection state machine (Connecting, Connected, Closing, Closed)
- Background task for connection health monitoring
- Configurable ping interval (default: 30s)

### 2. Message Framing

**Purpose**: Handle WebSocket protocol frame encoding/decoding

**Responsibilities**:
- Frame construction (RFC 6455)
- Fragmentation and reassembly
- Opcode handling (text, binary, ping, pong, close)
- Masking (for client-to-server frames)
- UTF-8 validation for text frames

**Frame Format**:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-------+-+-------------+-------------------------------+
|F|R|R|R| opcode|M| Payload len |    Extended payload length    |
|I|S|S|S|  (4)  |A|     (7)     |             (16/64)           |
|N|V|V|V|       |S|             |   (if payload len==126/127)   |
| |1|2|3|       |K|             |                               |
+-+-+-+-+-------+-+-------------+ - - - - - - - - - - - - - - - +
|     Extended payload length continued, if payload len == 127  |
+ - - - - - - - - - - - - - - - +-------------------------------+
|                               |Masking-key, if MASK set to 1  |
+-------------------------------+-------------------------------+
| Masking-key (continued)       |          Payload Data         |
+-------------------------------- - - - - - - - - - - - - - - - +
:                     Payload Data continued ...                :
+ - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - +
|                     Payload Data continued ...                |
+---------------------------------------------------------------+
```

**Implementation**:
```rust
pub struct Frame {
    fin: bool,
    rsv1: bool,
    rsv2: bool,
    rsv3: bool,
    opcode: OpCode,
    mask: bool,
    payload: Vec<u8>,
}

impl Frame {
    pub fn parse(bytes: &mut BytesMut) -> Result<Self>;
    pub fn encode(&self, buf: &mut BytesMut) -> Result<()>;
}
```

### 3. Stream Processing

**Purpose**: High-performance message stream processing

**Responsibilities**:
- Message batching
- Zero-copy optimization
- Backpressure signaling
- Flow control

**Implementation**:
```rust
pub struct MessageStream {
    socket: TcpStream,
    read_buf: BytesMut,
    write_buf: BytesMut,
    config: StreamConfig,
}

impl MessageStream {
    pub async fn send_message(&mut self, msg: Message) -> Result<()> {
        // Apply backpressure if needed
        if self.write_buf.len() > self.config.max_buffer_size {
            self.flush().await?;
        }

        // Encode frame
        let frame = Frame::from_message(msg);
        frame.encode(&mut self.write_buf)?;

        // Flush if buffer is full
        if self.write_buf.len() >= self.config.flush_threshold {
            self.flush().await?;
        }

        Ok(())
    }

    pub async fn recv_message(&mut self) -> Result<Message> {
        loop {
            // Try to parse frame from buffer
            if let Some(frame) = Frame::parse(&mut self.read_buf)? {
                return Ok(Message::from_frame(frame));
            }

            // Read more data
            let n = self.socket.read_buf(&mut self.read_buf).await?;
            if n == 0 {
                return Err(Error::ConnectionClosed);
            }
        }
    }
}
```

### 4. Compression

**Purpose**: Reduce message size for better throughput

**Algorithm**: Per-message DEFLATE compression (RFC 7692)

**Configuration**:
```rust
pub struct CompressionConfig {
    enabled: bool,
    level: CompressionLevel, // 0-9
    threshold: usize,        // Messages > threshold compressed
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: CompressionLevel::Fast,
            threshold: 1024, // Compress messages >1KB
        }
    }
}
```

**Trade-offs**:
- **Pros**: 2-5x reduction in size for text messages
- **Cons**: CPU overhead for compression/decompression
- **Recommendation**: Enable for messages >1KB

### 5. Backpressure Control

**Purpose**: Prevent overwhelming servers or clients

**Strategy**:

1. **Channel-based backpressure**
   - Bounded channels (default: 1000 messages)
   - Send blocks when channel is full
   - Prevents unbounded memory growth

2. **TCP-level backpressure**
   - Monitor kernel buffer space
   - Pause sends when buffer >80% full
   - Resume when buffer <60% full

3. **Application-level signals**
   - Notify application when backpressure active
   - Allow application to reduce send rate

**Implementation**:
```rust
pub struct BackpressureController {
    tx_buffer_size: usize,
    tx_buffer_limit: usize,
    backpressure_active: Arc<AtomicBool>,
}

impl BackpressureController {
    pub async fn send_with_backpressure(&self, msg: Message) -> Result<()> {
        // Block if backpressure active
        while self.is_backpressure_active() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Send message
        self.tx.send(msg).await?;

        Ok(())
    }

    fn is_backpressure_active(&self) -> bool {
        self.tx_buffer_size > (self.tx_buffer_limit * 80 / 100)
    }
}
```

### 6. Reconnection Logic

**Purpose**: Maintain connection despite network interruptions

**Strategy**:

1. **Exponential backoff**
   - Initial delay: 100ms
   - Max delay: 30s
   - Backoff multiplier: 1.5

2. **Connection state tracking**
   - Save last known connection state
   - Resubscribe to channels after reconnect
   - Replay missed messages (optional)

3. **Graceful degradation**
   - Buffer messages while disconnected
   - Apply backpressure if buffer fills
   - Drop old messages if buffer exceeds limit

**API**:
```rust
pub struct ReconnectConfig {
    enabled: bool,
    max_attempts: u32,     // 0 = infinite
    initial_delay: Duration,
    max_delay: Duration,
    backoff_multiplier: f64,
}

impl WebSocketClient {
    pub async fn wait_for_reconnection(&self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;

        while !self.is_connected() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if self.is_connected() {
            Ok(())
        } else {
            Err(Error::ReconnectTimeout)
        }
    }
}
```

### 7. Security

**Purpose**: Secure WebSocket connections

**Features**:

1. **TLS/SSL**
   - Native TLS support (rustls)
   - Certificate validation
   - ALPN protocol negotiation

2. **Authentication**
   - Token-based authentication
   - Custom authentication callbacks
   - Connection-level auth

3. **Authorization**
   - Per-message authorization hooks
   - Channel-based access control

**Implementation**:
```rust
pub struct SecurityConfig {
    tls: Option<TlsConfig>,
    auth: Option<AuthConfig>,
}

pub struct AuthConfig {
    token: Option<String>,
    validator: Option<Box<dyn AuthValidator>>,
}

#[async_trait]
pub trait AuthValidator: Send + Sync {
    async fn validate(&self, token: &str) -> Result<bool>;
}
```

### 8. Metrics and Telemetry

**Purpose**: Monitor performance and health

**Metrics Collected**:

1. **Connection metrics**
   - Active connections
   - Connection rate
   - Disconnection rate
   - Reconnection rate

2. **Message metrics**
   - Messages sent/received
   - Message latency (p50, p95, p99)
   - Message throughput
   - Error rate

3. **Resource metrics**
   - Memory usage
   - Buffer utilization
   - CPU usage (if available)

4. **Network metrics**
   - Bytes sent/received
   - Compression ratio
   - Retransmission rate

**Implementation**:
```rust
pub struct MetricsCollector {
    registry: Registry,
    connection_count: IntGauge,
    message_latency: Histogram,
    message_throughput: IntCounter,
}

impl MetricsCollector {
    pub fn record_latency(&self, latency: Duration) {
        let micros = latency.as_micros() as u64;
        self.message_latency.observe(micros as f64);
    }

    pub fn export_metrics(&self) -> String {
        // Export Prometheus format
        self.registry.gather()
    }
}
```

## WebSocket Protocol Implementation

### Handshake

**Client Request**:
```http
GET /chat HTTP/1.1
Host: server.example.com
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13
Sec-WebSocket-Protocol: chat, superchat
Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits
```

**Server Response**:
```http
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
Sec-WebSocket-Protocol: chat
Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits=15
```

### Frame Types

| Opcode | Type         | Description                       |
|--------|--------------|-----------------------------------|
| 0x0    | Continuation | Continuation of fragmented frame  |
| 0x1    | Text         | UTF-8 text data                   |
| 0x2    | Binary       | Binary data                       |
| 0x8    | Close        | Connection close                  |
| 0x9    | Ping         | Ping frame                        |
| 0xA    | Pong         | Pong frame                        |

### Fragmentation

**Large Message Handling**:

```
[FIN=0, OP=Text] "Hello, "
[FIN=0, OP=Cont] "Fragmented "
[FIN=1, OP=Cont] "World!"
```

**Implementation**:
```rust
pub async fn send_fragmented(&mut self, data: &str, fragment_size: usize) -> Result<()> {
    let chunks: Vec<_> = data.as_bytes().chunks(fragment_size).collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let fin = i == chunks.len() - 1;
        let opcode = if i == 0 { OpCode::Text } else { OpCode::Continuation };

        let frame = Frame {
            fin,
            opcode,
            payload: chunk.to_vec(),
            ..Default::default()
        };

        self.send_frame(frame).await?;
    }

    Ok(())
}
```

## Integration Points

### equilibrium-tokens Integration

**Message Flow**:

```
┌──────────────────┐
│ EquilibriumToken │
│   Orchestrator   │
└────────┬─────────┘
         │
         │ 1. Subscribe to token updates
         ▼
┌──────────────────┐      ┌─────────────┐
│ websocket-fabric │ ───▶ │   Client 1  │
│      Server      │      └─────────────┘
└────────┬─────────┘
         │
         │ 2. Broadcast token update
         ▼
┌──────────────────┐      ┌─────────────┐
│ websocket-fabric │ ───▶ │   Client 2  │
│   Broadcast      │      └─────────────┘
└──────────────────┘      └─────────────┘
```

**Integration Example**:
```rust
use websocket_fabric::{WebSocketServer, Message};
use equilibrium_tokens::EquilibriumOrchestrator;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup equilibrium orchestrator
    let orchestrator = EquilibriumOrchestrator::new(...).await?;

    // Setup WebSocket server
    let mut server = WebSocketServer::new("0.0.0.0:8080").await?;

    // Handle equilibrium-tokens messages
    server.on_message(move |msg| {
        let orchestrator = orchestrator.clone();
        async move {
            // Parse message
            let text = msg.as_text()?;
            let request: TokenRequest = serde_json::from_str(text)?;

            // Process with equilibrium orchestrator
            let response = orchestrator.handle_token_request(request).await?;

            // Return response
            Ok(Some(Message::json(response)?))
        }
    }).await?;

    // Subscribe to token updates
    let mut broadcast = server.broadcast_channel();
    tokio::spawn(async move {
        while let Some(update) = orchestrator.token_updates().recv().await {
            let msg = Message::json(update)?;
            broadcast.send(msg).await?;
        }
        Ok(())
    });

    server.serve().await?;

    Ok(())
}
```

## Performance Optimizations

### 1. Zero-Copy Message Passing

```rust
pub struct Message {
    // Use Bytes instead of Vec<u8> for zero-copy
    payload: Bytes,
}

// Pass by reference, avoid cloning
impl Message {
    pub fn as_bytes(&self) -> &[u8] {
        &self.payload
    }
}
```

### 2. Memory Pooling

```rust
use bytes::BytesMut;

// Reuse buffers
pub struct BufferPool {
    pool: Vec<BytesMut>,
}

impl BufferPool {
    pub fn acquire(&mut self) -> BytesMut {
        self.pool.pop().unwrap_or_else(|| BytesMut::with_capacity(4096))
    }

    pub fn release(&mut self, mut buf: BytesMut) {
        buf.clear();
        self.pool.push(buf);
    }
}
```

### 3. Batched Writes

```rust
pub async fn flush_batched(&mut self) -> Result<()> {
    if self.write_buf.is_empty() {
        return Ok(());
    }

    // Write entire buffer at once
    self.socket.write_all(&self.write_buf).await?;
    self.write_buf.clear();

    Ok(())
}
```

### 4. Lock-Free Structures

```rust
use crossbeam::queue::SegQueue;

// Lock-free message queue
pub struct MessageQueue {
    queue: SegQueue<Message>,
}

impl MessageQueue {
    pub fn push(&self, msg: Message) {
        self.queue.push(msg);
    }

    pub fn pop(&self) -> Option<Message> {
        self.queue.pop()
    }
}
```

## Error Handling

### Error Types

```rust
pub enum Error {
    // Connection errors
    ConnectionFailed,
    ConnectionClosed,
    ReconnectTimeout,

    // Message errors
    InvalidFrame,
    MessageTooLarge,
    Utf8Error,

    // Protocol errors
    ProtocolViolation,
    HandshakeFailed,

    // I/O errors
    Io(Arc<io::Error>),

    // Backpressure errors
    BufferFull,
    BackpressureTimeout,
}
```

### Error Recovery

```rust
impl WebSocketClient {
    pub async fn send_with_retry(&mut self, msg: Message, max_retries: u32) -> Result<()> {
        for attempt in 0..max_retries {
            match self.send(msg.clone()).await {
                Ok(_) => return Ok(()),
                Err(Error::BufferFull) if attempt < max_retries - 1 => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(Error::BackpressureTimeout)
    }
}
```

## Testing Strategy

See [TESTING_STRATEGY.md](TESTING_STRATEGY.md) for comprehensive testing approach.

## Configuration

### Server Configuration

```rust
pub struct ServerConfig {
    pub bind_address: SocketAddr,
    pub max_connections: usize,
    pub max_message_size: usize,
    pub ping_interval: Duration,
    pub ping_timeout: Duration,
    pub compression: CompressionConfig,
    pub backpressure: BackpressureConfig,
    pub security: SecurityConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:8080".parse().unwrap(),
            max_connections: 10_000,
            max_message_size: 10 * 1024 * 1024, // 10MB
            ping_interval: Duration::from_secs(30),
            ping_timeout: Duration::from_secs(10),
            compression: CompressionConfig::default(),
            backpressure: BackpressureConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}
```

### Client Configuration

```rust
pub struct ClientConfig {
    pub url: String,
    pub auto_reconnect: bool,
    pub reconnect_config: ReconnectConfig,
    pub max_message_size: usize,
    pub compression: CompressionConfig,
    pub security: SecurityConfig,
}
```

## Future Enhancements

1. **QUIC Support**: UDP-based transport for lower latency
2. **WebTransport**: Modern web transport protocol
3. **Multiplexing**: Multiple logical streams over one connection
4. **Custom codecs**: Plugin system for custom message codecs
5. **Advanced scheduling**: Quality of Service (QoS) for messages

## References

- [RFC 6455 - WebSocket Protocol](https://tools.ietf.org/html/rfc6455)
- [RFC 7692 - WebSocket Compression](https://tools.ietf.org/html/rfc7692)
- [Tokio Documentation](https://tokio.rs/)
- [Equilibrium Tokens](https://github.com/equilibrium-tokens)

# websocket-fabric Developer Guide

This guide is for contributors who want to help develop and maintain websocket-fabric.

## Table of Contents

1. [Development Setup](#development-setup)
2. [Project Structure](#project-structure)
3. [Architecture](#architecture)
4. [Coding Standards](#coding-standards)
5. [Testing](#testing)
6. [Benchmarks](#benchmarks)
7. [Documentation](#documentation)
8. [Pull Requests](#pull-requests)
9. [Release Process](#release-process)

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Git
- Linux/macOS/WSL2 (Windows support via WSL2)

### Fork and Clone

```bash
# Fork the repository on GitHub
# Clone your fork
git clone https://github.com/your-username/websocket-fabric.git
cd websocket-fabric

# Add upstream remote
git remote add upstream https://github.com/original-repo/websocket-fabric.git
```

### Install Dependencies

```bash
# Install Rust toolchain
rustup update

# Verify installation
rustc --version
cargo --version
```

### Build Project

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run clippy
cargo clippy --all-targets --all-features

# Format code
cargo fmt
```

## Project Structure

```
websocket-fabric/
├── Cargo.toml              # Package manifest
├── Cargo.lock              # Locked dependency versions
├── README.md               # Project overview
├── docs/                   # Documentation
│   ├── ARCHITECTURE.md     # Architecture documentation
│   ├── USER_GUIDE.md       # User guide
│   ├── DEVELOPER_GUIDE.md  # This file
│   ├── API.md              # API reference
│   └── TESTING_STRATEGY.md # Testing approach
├── src/                    # Source code
│   ├── lib.rs              # Library entry point
│   ├── client.rs           # WebSocket client
│   ├── message.rs          # Message types
│   ├── error.rs            # Error types
│   ├── config.rs           # Configuration
│   ├── reconnect.rs        # Reconnection logic
│   ├── heartbeat.rs        # Heartbeat/ping-pong
│   ├── backpressure.rs     # Backpressure control
│   └── metrics.rs          # Metrics collection
├── tests/                  # Integration tests
│   ├── integration_tests.rs
│   └── benches/            # Benchmarks
│       ├── message_bench.rs
│       └── connection_bench.rs
└── examples/               # Usage examples
    ├── basic_client.rs
    └── chat_client.rs
```

## Architecture

### Core Modules

1. **client.rs**: Main WebSocket client API
   - Connection lifecycle management
   - Message sending/receiving
   - Background task coordination

2. **message.rs**: Message types and framing
   - Message enum (Text, Binary, Ping, Pong, Close)
   - Zero-copy optimization with Bytes
   - JSON serialization support

3. **error.rs**: Error types
   - Comprehensive error enum
   - Error categorization (retryable, fatal)
   - Helpful error messages

4. **config.rs**: Configuration builders
   - ClientConfig, ServerConfig
   - ReconnectConfig, HeartbeatConfig, BackpressureConfig
   - Builder pattern for fluent API

5. **reconnect.rs**: Reconnection logic
   - Exponential backoff calculation
   - Reconnection state machine
   - Max attempts enforcement

6. **heartbeat.rs**: Ping/pong keepalive
   - Ping generation
   - Pong validation
   - Missed ping tracking

7. **backpressure.rs**: Flow control
   - Buffer utilization tracking
   - Threshold-based activation
   - Send throttling

8. **metrics.rs**: Telemetry
   - Connection stats
   - Message counts
   - Latency percentiles

### Key Design Principles

1. **Zero-Copy**: Use `Bytes` instead of `Vec<u8>` to avoid unnecessary copying
2. **Async-First**: Built on Tokio for efficient async I/O
3. **Type Safety**: Leverage Rust's type system for correctness
4. **Composability**: Small, focused modules with clear interfaces
5. **Observability**: Comprehensive metrics for debugging and monitoring

## Coding Standards

### Code Style

We follow standard Rust style guidelines:

```bash
# Format code
cargo fmt

# Check formatting without making changes
cargo fmt --check
```

### Linting

All code must pass clippy without warnings:

```bash
# Run clippy
cargo clippy --all-targets --all-features

# Fix clippy suggestions automatically
cargo clippy --fix --all-targets --all-features
```

### Naming Conventions

- **Structs**: `PascalCase` - `WebSocketClient`, `Message`
- **Functions**: `snake_case` - `send_text`, `connect_with_config`
- **Constants**: `SCREAMING_SNAKE_CASE` - `DEFAULT_MAX_MESSAGE_SIZE`
- **Types**: `PascalCase` - `ClientConfig`, `Error`

### Documentation

All public items must have documentation:

```rust
/// Send a text message.
///
/// # Arguments
///
/// * `text` - The text message to send
///
/// # Returns
///
/// Returns `Ok(())` if the message was sent successfully.
///
/// # Errors
///
/// Returns `Error::BufferFull` if backpressure is active.
/// Returns `Error::ConnectionClosed` if the connection is closed.
///
/// # Examples
///
/// ```no_run
/// use websocket_fabric::WebSocketClient;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = WebSocketClient::connect("ws://localhost:8080").await?;
/// client.send_text("Hello, World!").await?;
/// # Ok(())
/// # }
/// ```
pub async fn send_text(&self, text: &str) -> Result<()> {
    // ...
}
```

### Error Handling

- Use `Result<T, Error>` for fallible operations
- Provide context with error messages
- Categorize errors as retryable or fatal
- Use `thiserror` for error derives

```rust
use crate::error::{Error, Result};

pub async fn send_message(&self, msg: Message) -> Result<()> {
    // Check backpressure
    if self.backpressure.is_backpressure_active() {
        return Err(Error::BufferFull);
    }

    // Send message
    self.tx.send(msg)
        .await
        .map_err(|_| Error::ConnectionClosed)?;

    Ok(())
}
```

### Async/Await

- Use `async fn` for async functions
- Prefer `.await` over blocking operations
- Use `tokio::spawn` for concurrent tasks

```rust
// GOOD: Async function
pub async fn connect(url: &str) -> Result<Self> {
    let (stream, _) = connect_async(url).await?;
    // ...
}

// BAD: Blocking in async function
pub async fn connect(url: &str) -> Result<Self> {
    let (stream, _) = connect_async(url).await?; // OK
    std::thread::sleep(Duration::from_secs(1)); // BAD: blocks executor
    // ...
}

// GOOD: Async sleep
pub async fn connect(url: &str) -> Result<Self> {
    let (stream, _) = connect_async(url).await?;
    tokio::time::sleep(Duration::from_secs(1)).await; // GOOD
    // ...
}
```

## Testing

### Unit Tests

Write unit tests in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::text("Hello");
        assert!(msg.is_text());
        assert_eq!(msg.as_text().unwrap(), "Hello");
    }

    #[tokio::test]
    async fn test_client_connection() {
        let client = WebSocketClient::connect("ws://localhost:8080").await;
        assert!(client.is_ok());
    }
}
```

### Integration Tests

Add integration tests in `tests/` directory:

```rust
// tests/integration_tests.rs
use websocket_fabric::WebSocketClient;

#[tokio::test]
async fn test_echo_server() {
    let client = WebSocketClient::connect("ws://echo.websocket.org").await.unwrap();

    client.send_text("Hello, Echo!").await.unwrap();

    // Wait for echo response (implementation dependent)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    client.close(None).await.unwrap();
}
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_message_creation

# Run tests in release mode (faster)
cargo test --release

# Run ignored tests
cargo test -- --ignored
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html
```

### Test Guidelines

1. **Test behavior, not implementation**: Focus on what the code does, not how
2. **Test edge cases**: Empty inputs, maximum values, error conditions
3. **Use property-based testing**: Use `proptest` for randomized testing
4. **Mock external dependencies**: Use test doubles for network I/O
5. **Keep tests fast**: Unit tests should run in <100ms

### Property-Based Testing

```rust
// Add to Cargo.toml
[dev-dependencies]
proptest = "1.0"

// In tests
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_roundtrip(text in ".*") {
        let msg = Message::text(&text);
        assert_eq!(msg.as_text().unwrap(), text);
    }
}
```

## Benchmarks

### Writing Benchmarks

Add benchmarks in `tests/benches/`:

```rust
// tests/benches/message_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use websocket_fabric::Message;

fn bench_message_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_creation");

    for size in [10, 100, 1000, 10000].iter() {
        let data = vec![0u8; *size];

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(Message::binary(data.clone()));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_message_creation);
criterion_main!(benches);
```

### Running Benchmarks

```bash
# Run benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench message_bench

# Generate plots (requires gnuplot)
cargo bench -- --save-baseline main
cargo bench -- --baseline main --plot
```

### Benchmark Guidelines

1. **Use Criterion**: Provides statistical analysis and plotting
2. **Compare baselines**: Track performance over time
3. **Test realistic workloads**: Benchmark actual use cases
4. **Avoid compiler optimizations**: Use `black_box` to prevent optimizations
5. **Run multiple times**: Ensure results are consistent

## Documentation

### Rustdoc

All public APIs must have rustdoc comments:

```rust
/// Connect to a WebSocket server.
///
/// This function establishes a WebSocket connection to the specified URL.
///
/// # Arguments
///
/// * `url` - The WebSocket URL to connect to (e.g., `ws://localhost:8080`)
///
/// # Returns
///
/// Returns a `WebSocketClient` instance on success.
///
/// # Errors
///
/// This function will return an error if:
/// - The URL is invalid
/// - The connection fails
/// - The handshake times out
///
/// # Examples
///
/// ```no_run
/// use websocket_fabric::WebSocketClient;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = WebSocketClient::connect("ws://localhost:8080").await?;
/// # Ok(())
/// # }
/// ```
pub async fn connect(url: impl Into<String>) -> Result<Self> {
    // ...
}
```

### Documentation Examples

All code examples in documentation must compile and run:

```bash
# Test documentation examples
cargo test --doc

# Open documentation in browser
cargo doc --open
```

### Markdown Documentation

Maintain these documentation files:
- `README.md`: Quick start and overview
- `docs/ARCHITECTURE.md`: Architecture documentation
- `docs/USER_GUIDE.md`: User guide with examples
- `docs/DEVELOPER_GUIDE.md`: This file
- `docs/API.md`: API reference

## Pull Requests

### Before Submitting

1. **Update documentation**: Include relevant changes in docs
2. **Add tests**: Ensure new code is tested
3. **Run clippy**: Fix all warnings
4. **Format code**: Run `cargo fmt`
5. **Update CHANGELOG**: Add entry for your changes

### PR Checklist

- [ ] Code compiles without warnings
- [ ] All tests pass (`cargo test`)
- [ ] Clippy passes (`cargo clippy`)
- [ ] Documentation updated
- [ ] Tests added/updated
- [ ] CHANGELOG updated

### PR Description Template

```markdown
## Description
Brief description of the changes

## Motivation
Why this change is needed

## Testing
How this was tested

## Checklist
- [ ] Documentation updated
- [ ] Tests added
- [ ] All tests pass
- [ ] Clippy clean
```

## Release Process

### Versioning

We follow Semantic Versioning 2.0.0:
- **MAJOR**: Incompatible API changes
- **MINOR**: Backwards-compatible functionality
- **PATCH**: Backwards-compatible bug fixes

### Pre-Release Checklist

1. All tests pass
2. Documentation complete
3. CHANGELOG updated
4. Version number updated in `Cargo.toml`
5. Git tag created

### Release Steps

```bash
# Update version in Cargo.toml
# Update CHANGELOG.md

# Commit changes
git commit -am "Release v0.2.0"

# Create tag
git tag -a v0.2.0 -m "Release v0.2.0"

# Push to GitHub
git push origin main
git push origin v0.2.0

# Publish to crates.io
cargo publish
```

### Post-Release

1. Announce release on GitHub Releases
2. Update documentation website
3. Create discussion post
4. Monitor for issues

## Contributing

### Ways to Contribute

- **Bug fixes**: Always welcome
- **New features**: Open issue first for discussion
- **Documentation**: Improving docs is valuable
- **Tests**: More test coverage is always good
- **Benchmarks**: Performance improvements welcome

### Getting Help

- Open a GitHub issue for bugs or feature requests
- Start a GitHub discussion for questions
- Check existing issues and discussions first

### Code Review Process

1. Open pull request
2. Automatic CI checks run
3. Maintainer reviews code
4. Address feedback
5. Approval and merge

### Community Guidelines

- Be respectful and constructive
- Focus on what is best for the community
- Show empathy towards other community members

## License

By contributing, you agree that your contributions will be licensed under the MIT OR Apache-2.0 license.

Thank you for contributing to websocket-fabric!

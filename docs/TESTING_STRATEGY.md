# Testing Strategy for websocket-fabric

## Overview

This document outlines the comprehensive testing strategy for **websocket-fabric**, a high-performance WebSocket library designed for sub-millisecond latency real-time communication. The strategy ensures reliability, performance, and seamless integration with equilibrium-tokens and other systems.

## Testing Philosophy

- **Fast Feedback**: Unit tests complete in <10 seconds
- **Realistic Scenarios**: Integration tests mirror production usage
- **Performance-First**: All code changes must meet performance targets
- **Continuous Validation**: CI/CD runs tests at multiple frequencies

## Test Categories

### 1. Unit Tests

**Location**: `tests/connection_tests.rs`, `tests/message_tests.rs`

**Goal**: Validate individual components and functions in isolation

**Runtime**: <10 seconds

**Execution**: Every PR, every commit

**Coverage**: >90%

#### Connection Tests
- WebSocket handshake validation
- Connection lifecycle (connect, ping, close)
- Reconnection logic
- Graceful shutdown
- Connection state transitions
- Timeout handling
- Concurrent connections

#### Message Tests
- Text message send/receive
- Binary message handling
- Fragmented messages
- Ping/pong frames
- Message ordering guarantees
- Concurrent send/receive
- Message size limits
- UTF-8 validation
- Empty messages
- Message compression

#### Running Unit Tests

```bash
# Run all unit tests
cargo test --lib

# Run specific test file
cargo test --test connection_tests

# Run with output
cargo test -- --nocapture

# Run tests in parallel
cargo test -- --test-threads=4
```

### 2. Integration Tests

**Location**: `tests/integration_tests.rs`

**Goal**: Validate integration with equilibrium-tokens and real-world scenarios

**Runtime**: <30 seconds

**Execution**: Every PR, nightly

#### Integration Scenarios

**equilibrium-tokens Integration**
```rust
test_equilibrium_integration()
```
- Validates message flow between websocket-fabric and equilibrium-tokens
- Tests token operation messages (mint, transfer, burn)
- Verifies response handling

**Backpressure Handling**
```rust
test_backpressure_handling()
```
- Client sends faster than server can process
- Server applies backpressure without crashing
- Client handles backpressure gracefully
- System recovers when load decreases

**Real-time Updates**
```rust
test_real_time_token_updates()
test_multi_client_synchronization()
```
- Token price updates broadcast to all subscribers
- Multi-client synchronization
- Subscription management

**Error Propagation**
```rust
test_error_propagation()
```
- Errors from equilibrium-tokens propagate to clients
- Proper error codes and messages
- Error recovery

**Authentication**
```rust
test_authentication_flow()
```
- Token-based authentication
- Invalid token rejection
- Connection establishment with auth

**Message Acknowledgment**
```rust
test_message_acknowledgment()
```
- Critical operation acknowledgments
- Exactly-once delivery semantics
- State tracking

**State Recovery**
```rust
test_reconnection_with_state_recovery()
```
- Automatic reconnection
- State persistence across reconnections
- Session restoration

#### Running Integration Tests

```bash
# Run all integration tests
cargo test --test integration_tests

# Run specific integration test
cargo test test_equilibrium_integration

# Run with logging
RUST_LOG=debug cargo test --test integration_tests
```

### 3. Performance Tests

**Location**: `tests/performance_tests.rs`

**Goal**: Validate performance targets and identify regressions

**Runtime**: <1 minute

**Execution**: Every PR, nightly

#### Performance Targets

**Message Latency**
- **P50**: <100µs
- **P95**: <500µs
- **P99**: <1ms

```rust
test_message_latency_p50()
test_message_latency_p95()
test_message_latency_p99()
```

**Throughput**
- **Per connection**: >100K messages/sec
- **Server total**: >1M messages/sec

```rust
test_throughput_per_connection()
test_throughput_multiple_connections()
test_binary_message_throughput()
```

**Memory Usage**
- **Per connection**: <10KB

```rust
test_memory_per_connection()
```

**Additional Metrics**
- Ping/pong latency: <100µs
- Connection establishment: <10ms
- Concurrent send performance

#### Running Performance Tests

```bash
# Run performance tests
cargo test --test performance_tests

# Run with release optimizations
cargo test --release --test performance_tests

# Measure specific metric
cargo test test_message_latency_p50 -- --nocapture
```

### 4. Benchmarks

**Location**: `tests/benches/`

**Goal**: Detailed performance profiling and optimization

**Runtime**: ~10 minutes (full suite)

**Execution**: Nightly, before releases

#### Benchmark Categories

**Message Latency** (`message_latency.rs`)
- Single message latency
- Latency by message size (16B to 4KB)
- Ping/pong latency
- Round-trip latency
- Concurrent operation latency

**Throughput** (`throughput.rs`)
- Unidirectional throughput
- Bidirectional throughput
- Binary throughput (by size)
- Multi-connection throughput
- Small message throughput

**Concurrent Connections** (`concurrent_connections.rs`)
- Concurrent connection handling
- Concurrent message exchange
- Connection establishment rate
- Broadcast throughput

**Memory Usage** (`memory_usage.rs`)
- Memory per connection
- Message buffering memory
- Large message memory
- Memory fragmentation
- Buffer reuse efficiency

#### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench message_latency

# Run with custom settings
cargo bench -- --sample-size 100 --warm-up-time 5 --measurement-time 10

# Generate plots (requires gnuplot)
cargo bench -- --plotting-backend gnuplot
```

#### Benchmark Analysis

Benchmarks use Criterion for accurate measurements:

```bash
# Compare against previous baseline
cargo bench -- --save-baseline main
# Make changes
cargo bench -- --baseline main

# Generate HTML report
cargo bench -- --output-format bencher | tee benchmark.txt
```

### 5. Stress Tests

**Location**: `tests/stress_tests.rs`

**Goal**: Validate system behavior under extreme load

**Runtime**: Variable (minutes to hours)

**Execution**: Nightly, weekly, pre-release

#### Stress Test Scenarios

**10K Concurrent Connections**
```rust
test_concurrent_connections_10k()
```
- 10,000 simultaneous connections
- Message exchange under load
- Server responsiveness validation
- Memory usage monitoring

**Memory Leak Detection**
```rust
test_memory_leak_long_running()
```
- 1M message iterations
- Memory growth monitoring
- Leak detection and prevention

**Message Bursts**
```rust
test_message_burst()
```
- 100K message bursts
- Buffer management
- System recovery

**Connection Churn**
```rust
test_rapid_connect_disconnect()
test_churn_connections()
```
- 10K connect/disconnect cycles
- Connection churn under load
- Resource cleanup validation

**Large Messages**
```rust
test_large_message_handling()
```
- Up to 10MB messages
- Progressive size testing
- Throughput validation

**Server Restart Under Load**
```rust
test_server_restart_under_load()
```
- Server restart with 1K active connections
- Reconnection behavior
- State recovery

**24-Hour Stability**
```rust
test_24_hour_stability()
```
- Extended run validation
- Memory stability
- Connection stability

**Mixed Workload**
```rust
test_mixed_workload()
```
- 100K mixed operations (text, binary, ping, fragmented)
- Real-world scenario simulation

#### Running Stress Tests

```bash
# Run all stress tests (ignored by default)
cargo test --test stress_tests -- --ignored

# Run specific stress test
cargo test test_concurrent_connections_10k -- --ignored --nocapture

# Run with release (recommended)
cargo test --release --test stress_tests -- --ignored

# Run with time limit
timeout 3600 cargo test --release --test stress_tests -- --ignored
```

#### Stress Test Monitoring

Monitor during stress tests:

```bash
# In one terminal: run stress test
cargo test --release --test stress_tests -- --ignored --nocapture

# In another: monitor resources
watch -n 1 'ps aux | grep websocket-fabric'
watch -n 1 'ss -t | grep :8080 | wc -l'
```

## CI/CD Integration

### Every PR (Fast Feedback)

**Duration**: <2 minutes

```yaml
# .github/workflows/pr.yml
steps:
  - name: Run unit tests
    run: cargo test --lib

  - name: Run integration tests
    run: cargo test --test integration_tests

  - name: Run performance tests
    run: cargo test --test performance_tests
```

### Nightly Builds

**Duration**: ~15 minutes

```yaml
# .github/workflows/nightly.yml
steps:
  - name: Run full benchmark suite
    run: cargo bench

  - name: Run stress tests (subset)
    run: cargo test --test stress_tests -- --ignored

  - name: Check memory leaks
    run: cargo test test_memory_leak_long_running -- --ignored --nocapture
```

### Weekly Builds

**Duration**: ~2 hours

```yaml
# .github/workflows/weekly.yml
steps:
  - name: Extended load testing
    run: cargo test test_concurrent_connections_10k -- --ignored

  - name: 24-hour stability
    run: cargo test test_24_hour_stability -- --ignored --nocapture

  - name: Full stress suite
    run: cargo test --release --test stress_tests -- --ignored
```

### Performance Regression Detection

Automated performance regression detection:

```bash
# Store baseline performance
cargo bench -- --save-baseline main

# Compare on PR
cargo bench -- --baseline main

# Fail if regressed
# (configured in .cargo/config.toml or CI script)
```

## Test Data and Fixtures

### Mock Server

```rust
// tests/common/mod.rs
pub async fn setup_test_server() -> WebSocketServer {
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    server.spawn().await.unwrap();
    server
}
```

### Test Messages

```rust
pub const EQUILIBRIUM_TOKEN_REQUEST: &str = r#"{
    "type": "token_request",
    "token_id": "eq_123",
    "action": "mint"
}"#;
```

## Coverage Goals

- **Unit tests**: >90% code coverage
- **Integration tests**: Cover all equilibrium-tokens integration points
- **Performance tests**: Cover all critical paths
- **Stress tests**: Cover all failure modes

## Monitoring and Alerts

### Metrics to Track

- Test duration trends
- Memory usage trends
- Performance regression alerts
- Flaky test detection

### Alert Thresholds

- Test failure: Immediate alert
- Performance regression >10%: Warning
- Performance regression >20%: Block PR
- Memory growth >50%: Warning

## Documentation

All tests include:

- Clear documentation comments
- Explanation of what is being tested
- Expected vs actual behavior
- Setup and teardown procedures

## Best Practices

1. **Isolation**: Each test is independent and can run in any order
2. **Determinism**: Tests use fixed seeds and predictable inputs
3. **Speed**: Tests run as fast as possible (mock external dependencies)
4. **Clarity**: Test names clearly describe what is being tested
5. **Maintenance**: Tests are easy to understand and modify

## Continuous Improvement

- Review test coverage monthly
- Add tests for bugs found in production
- Optimize slow tests
- Update performance targets as needed
- Refactor duplicate test code

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Criterion Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)

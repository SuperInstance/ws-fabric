//! Example demonstrating rate limiting with websocket-fabric
//!
//! This example shows how to use the rate limiting features to control
//! the rate of WebSocket messages, bytes, and operations.

use websocket_fabric::ratelimit::{RateLimiter, RateLimitType, RateLimitConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== WebSocket Fabric Rate Limiting Example ===\n");

    // Example 1: Basic rate limiting
    println!("Example 1: Basic rate limiting");
    let config = RateLimitConfig::new()
        .with_messages_per_second(10)
        .with_bytes_per_second(1024 * 1024) // 1MB/s
        .with_operations_per_second(100)
        .with_burst_size(20);

    let limiter = RateLimiter::new(config.clone());

    println!("Configured limits:");
    println!("  Messages: {} per second (burst: {})", config.messages_per_second, config.burst_size);
    println!("  Bytes: {} per second (burst: {})", config.bytes_per_second, config.burst_size);
    println!("  Operations: {} per second (burst: {})", config.operations_per_second, config.burst_size);
    println!();

    // Example 2: Try to acquire permits (non-blocking)
    println!("Example 2: Non-blocking acquire");
    for i in 0..25 {
        match limiter.try_acquire(RateLimitType::Message, 1) {
            Ok(()) => println!("  Message {} allowed", i + 1),
            Err(_) => println!("  Message {} blocked (rate limit exceeded)", i + 1),
        }
    }
    println!();

    // Example 3: Check rate limit status
    println!("Example 3: Check rate limit status");
    let status = limiter.status(RateLimitType::Message);
    println!("  Remaining messages: {}", status.remaining);
    println!("  Capacity: {}", status.capacity);
    println!("  Limit per second: {}", status.limit);
    println!("  Resets in: {:?}", status.resets_in);
    println!();

    // Example 4: Wait for permit (blocking)
    println!("Example 4: Blocking acquire (waits for tokens)");
    println!("  Attempting to acquire 1 message permit...");
    let start = std::time::Instant::now();
    limiter.acquire(RateLimitType::Message, 1).await?;
    let elapsed = start.elapsed();
    println!("  Permit acquired after {:?}", elapsed);
    println!();

    // Example 5: Reset rate limits
    println!("Example 5: Reset rate limits");
    limiter.reset();
    let status = limiter.status(RateLimitType::Message);
    println!("  After reset, remaining messages: {}", status.remaining);
    println!();

    // Example 6: Byte rate limiting
    println!("Example 6: Byte rate limiting");
    let byte_config = RateLimitConfig::new()
        .with_bytes_per_second(2048)
        .with_burst_size(4096);

    let byte_limiter = RateLimiter::new(byte_config);

    // Try to send 3000 bytes
    match byte_limiter.try_acquire(RateLimitType::Byte, 3000) {
        Ok(()) => println!("  3000 bytes allowed"),
        Err(_) => println!("  3000 bytes blocked"),
    }

    let status = byte_limiter.status(RateLimitType::Byte);
    println!("  Remaining bytes: {}", status.remaining);
    println!();

    // Example 7: Disabled rate limiting
    println!("Example 7: Disabled rate limiting");
    let no_limit = RateLimiter::disabled();
    println!("  Rate limiting enabled: {}", no_limit.is_enabled());

    // Should be able to acquire any amount
    for _ in 0..10000 {
        if no_limit.try_acquire(RateLimitType::Message, 1).is_err() {
            println!("  ERROR: Should not be rate limited!");
            break;
        }
    }
    println!("  Successfully acquired 10000 message permits (no limit)");
    println!();

    println!("=== Example Complete ===");

    Ok(())
}

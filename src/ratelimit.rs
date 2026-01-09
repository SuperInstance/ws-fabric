//! Rate limiting for WebSocket connections using token bucket algorithm
//!
//! This module provides rate limiting capabilities to prevent abuse and control
//! resource usage. It implements the token bucket algorithm which allows bursts
//! while maintaining a long-term rate limit.
//!
//! # Algorithm
//!
//! The token bucket algorithm works as follows:
//! - A bucket has a maximum capacity (burst size)
//! - Tokens are added to the bucket at a constant rate
//! - Each operation consumes one or more tokens
//! - If the bucket is empty, operations are blocked
//! - The bucket can accumulate tokens up to its capacity, allowing bursts
//!
//! # Example
//!
//! ```rust
//! use websocket_fabric::ratelimit::{RateLimiter, RateLimitType, RateLimitConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a rate limiter
//! let config = RateLimitConfig::new()
//!     .with_messages_per_second(100)
//!     .with_bytes_per_second(1024 * 1024) // 1MB/s
//!     .with_burst_size(200);
//!
//! let limiter = RateLimiter::new(config);
//!
//! // Check if operation is allowed (non-blocking)
//! match limiter.try_acquire(RateLimitType::Message, 1) {
//!     Ok(()) => {
//!         // Send message
//!     }
//!     Err(_) => {
//!         // Rate limit exceeded
//!     }
//! }
//!
//! // Wait for permit (blocking)
//! limiter.acquire(RateLimitType::Message, 1).await;
//!
//! // Get rate limit status
//! let status = limiter.status(RateLimitType::Message);
//! println!("Remaining: {}", status.remaining);
//! println!("Resets in: {:?}", status.resets_in);
//! # Ok(())
//! # }
//! ```

use crate::error::{Error, Result};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Rate limit type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitType {
    /// Rate limit by message count
    Message,
    /// Rate limit by byte count
    Byte,
    /// Rate limit by operation count
    Operation,
}

impl RateLimitType {
    /// Get all rate limit types
    pub fn all() -> &'static [RateLimitType] {
        &[RateLimitType::Message, RateLimitType::Byte, RateLimitType::Operation]
    }
}

/// Rate limit status
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    /// Remaining tokens
    pub remaining: u64,
    /// Time until bucket refills to capacity
    pub resets_in: Duration,
    /// Rate limit (tokens per second)
    pub limit: u64,
    /// Burst capacity
    pub capacity: u64,
}

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum messages per second
    pub messages_per_second: u64,
    /// Maximum bytes per second
    pub bytes_per_second: u64,
    /// Maximum operations per second
    pub operations_per_second: u64,
    /// Burst size (maximum token accumulation)
    pub burst_size: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            messages_per_second: 100,
            bytes_per_second: 1024 * 1024, // 1MB/s
            operations_per_second: 1000,
            burst_size: 100,
        }
    }
}

impl RateLimitConfig {
    /// Create a new rate limit configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set messages per second limit
    pub fn with_messages_per_second(mut self, limit: u64) -> Self {
        self.messages_per_second = limit;
        self
    }

    /// Set bytes per second limit
    pub fn with_bytes_per_second(mut self, limit: u64) -> Self {
        self.bytes_per_second = limit;
        self
    }

    /// Set operations per second limit
    pub fn with_operations_per_second(mut self, limit: u64) -> Self {
        self.operations_per_second = limit;
        self
    }

    /// Set burst size (maximum token accumulation)
    pub fn with_burst_size(mut self, size: u64) -> Self {
        self.burst_size = size;
        self
    }

    /// Disable rate limiting
    pub fn disabled() -> Self {
        Self {
            messages_per_second: u64::MAX,
            bytes_per_second: u64::MAX,
            operations_per_second: u64::MAX,
            burst_size: u64::MAX,
        }
    }

    /// Get limit for a specific type
    fn get_limit(&self, rate_limit_type: RateLimitType) -> u64 {
        match rate_limit_type {
            RateLimitType::Message => self.messages_per_second,
            RateLimitType::Byte => self.bytes_per_second,
            RateLimitType::Operation => self.operations_per_second,
        }
    }
}

/// Token bucket state
#[derive(Debug)]
struct TokenBucket {
    /// Current token count
    tokens: AtomicU64,
    /// Last refill timestamp
    last_refill: AtomicU64,
    /// Maximum capacity (burst size)
    capacity: u64,
    /// Refill rate (tokens per second)
    refill_rate: u64,
}

impl TokenBucket {
    /// Create a new token bucket
    fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            tokens: AtomicU64::new(capacity),
            last_refill: AtomicU64::new(timestamp_nanos()),
            capacity,
            refill_rate,
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&self) {
        let now = timestamp_nanos();
        let last = self.last_refill.load(Ordering::Relaxed);
        let elapsed_nanos = now.saturating_sub(last);

        if elapsed_nanos == 0 {
            return;
        }

        // Calculate tokens to add: (elapsed / 1e9) * refill_rate
        let elapsed_seconds = elapsed_nanos as f64 / 1_000_000_000.0;
        let tokens_to_add = (elapsed_seconds * self.refill_rate as f64) as u64;

        if tokens_to_add > 0 {
            // Add tokens up to capacity
            let current = self.tokens.load(Ordering::Relaxed);
            let new_tokens = current.saturating_add(tokens_to_add).min(self.capacity);
            self.tokens.store(new_tokens, Ordering::Relaxed);

            // Update last refill time, but don't move it forward
            // more than the tokens we actually added
            let added_tokens = new_tokens.saturating_sub(current);
            let added_nanos = (added_tokens as f64 / self.refill_rate as f64 * 1_000_000_000.0) as u64;
            self.last_refill.store(last.saturating_add(added_nanos.min(elapsed_nanos)), Ordering::Relaxed);
        }
    }

    /// Try to consume tokens (non-blocking)
    fn try_consume(&self, amount: u64) -> bool {
        self.refill();

        let mut current = self.tokens.load(Ordering::Acquire);

        loop {
            if current < amount {
                return false;
            }

            let new = current.saturating_sub(amount);
            match self.tokens.compare_exchange_weak(
                current,
                new,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get current token count
    fn available(&self) -> u64 {
        self.refill();
        self.tokens.load(Ordering::Relaxed)
    }

    /// Get capacity
    fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get refill rate
    fn refill_rate(&self) -> u64 {
        self.refill_rate
    }

    /// Calculate time until bucket is full
    fn time_until_full(&self) -> Duration {
        let available = self.available();
        if available >= self.capacity {
            return Duration::ZERO;
        }

        let needed = self.capacity - available;
        if self.refill_rate == 0 {
            return Duration::from_secs(60); // Default to 1 minute if no refill
        }

        let seconds_needed = needed as f64 / self.refill_rate as f64;
        Duration::from_secs_f64(seconds_needed)
    }

    /// Reset bucket to full capacity
    fn reset(&self) {
        self.tokens.store(self.capacity, Ordering::Relaxed);
        self.last_refill.store(timestamp_nanos(), Ordering::Relaxed);
    }
}

/// Get current timestamp in nanoseconds
fn timestamp_nanos() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Rate limiter using token bucket algorithm
#[derive(Debug, Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<Mutex<std::collections::HashMap<RateLimitType, Arc<TokenBucket>>>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        let mut buckets = std::collections::HashMap::new();

        for &rate_limit_type in RateLimitType::all() {
            let limit = config.get_limit(rate_limit_type);
            // Use burst_size as capacity, but ensure it's at least 1 for all types
            // For bytes, if burst_size is too small (like default 100), scale it up
            let capacity = match rate_limit_type {
                RateLimitType::Byte => config.burst_size.max(1024), // At least 1KB for bytes
                _ => config.burst_size.max(1), // At least 1 for messages/operations
            };
            buckets.insert(rate_limit_type, Arc::new(TokenBucket::new(capacity, limit)));
        }

        Self {
            config,
            buckets: Arc::new(Mutex::new(buckets)),
        }
    }

    /// Create a rate limiter with default configuration
    pub fn default_config() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Create a disabled rate limiter (no limits)
    pub fn disabled() -> Self {
        Self::new(RateLimitConfig::disabled())
    }

    /// Try to acquire a permit (non-blocking)
    ///
    /// Returns Ok(()) if the operation is allowed, Err otherwise.
    pub fn try_acquire(&self, rate_limit_type: RateLimitType, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }

        let buckets = self.buckets.lock();
        let bucket = buckets.get(&rate_limit_type)
            .ok_or_else(|| Error::invalid_state("Rate limit type not configured"))?;

        if bucket.try_consume(amount) {
            Ok(())
        } else {
            Err(Error::RateLimitExceeded {
                rate_limit_type,
                retry_after: bucket.time_until_full(),
            })
        }
    }

    /// Acquire a permit (blocking)
    ///
    /// This will wait until a permit is available.
    pub async fn acquire(&self, rate_limit_type: RateLimitType, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }

        loop {
            match self.try_acquire(rate_limit_type, amount) {
                Ok(()) => return Ok(()),
                Err(Error::RateLimitExceeded { retry_after, .. }) => {
                    tokio::time::sleep(retry_after).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Get rate limit status for a specific type
    pub fn status(&self, rate_limit_type: RateLimitType) -> RateLimitStatus {
        let buckets = self.buckets.lock();
        let bucket = buckets.get(&rate_limit_type);

        match bucket {
            Some(bucket) => {
                let remaining = bucket.available();
                RateLimitStatus {
                    remaining,
                    resets_in: bucket.time_until_full(),
                    limit: bucket.refill_rate(),
                    capacity: bucket.capacity(),
                }
            }
            None => RateLimitStatus {
                remaining: 0,
                resets_in: Duration::ZERO,
                limit: 0,
                capacity: 0,
            },
        }
    }

    /// Reset all rate limit counters
    pub fn reset(&self) {
        let buckets = self.buckets.lock();
        for bucket in buckets.values() {
            bucket.reset();
        }
    }

    /// Reset rate limit counter for a specific type
    pub fn reset_type(&self, rate_limit_type: RateLimitType) {
        let buckets = self.buckets.lock();
        if let Some(bucket) = buckets.get(&rate_limit_type) {
            bucket.reset();
        }
    }

    /// Check if rate limiting is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.messages_per_second != u64::MAX
    }

    /// Get configuration
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::default_config()
    }
}

/// Global rate limiter for connection pooling
#[derive(Debug, Clone)]
pub struct GlobalRateLimiter {
    limiter: RateLimiter,
    connection_count: Arc<AtomicU64>,
    max_connections: u64,
}

impl GlobalRateLimiter {
    /// Create a new global rate limiter
    pub fn new(config: RateLimitConfig, max_connections: u64) -> Self {
        Self {
            limiter: RateLimiter::new(config),
            connection_count: Arc::new(AtomicU64::new(0)),
            max_connections,
        }
    }

    /// Create a global rate limiter with default configuration
    pub fn default_config(max_connections: u64) -> Self {
        Self::new(RateLimitConfig::default(), max_connections)
    }

    /// Try to acquire a permit (non-blocking)
    pub fn try_acquire(&self, rate_limit_type: RateLimitType, amount: u64) -> Result<()> {
        self.limiter.try_acquire(rate_limit_type, amount)
    }

    /// Acquire a permit (blocking)
    pub async fn acquire(&self, rate_limit_type: RateLimitType, amount: u64) -> Result<()> {
        self.limiter.acquire(rate_limit_type, amount).await
    }

    /// Get rate limit status
    pub fn status(&self, rate_limit_type: RateLimitType) -> RateLimitStatus {
        self.limiter.status(rate_limit_type)
    }

    /// Add a connection
    pub fn add_connection(&self) -> Result<()> {
        let count = self.connection_count.fetch_add(1, Ordering::Relaxed);
        if count + 1 > self.max_connections {
            self.connection_count.fetch_sub(1, Ordering::Relaxed);
            Err(Error::ConnectionLimitExceeded {
                current: count + 1,
                limit: self.max_connections,
            })
        } else {
            Ok(())
        }
    }

    /// Remove a connection
    pub fn remove_connection(&self) {
        self.connection_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current connection count
    pub fn connection_count(&self) -> u64 {
        self.connection_count.load(Ordering::Relaxed)
    }

    /// Check if rate limiting is enabled
    pub fn is_enabled(&self) -> bool {
        self.limiter.is_enabled()
    }

    /// Reset all rate limit counters
    pub fn reset(&self) {
        self.limiter.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.messages_per_second, 100);
        assert_eq!(config.bytes_per_second, 1024 * 1024);
        assert_eq!(config.operations_per_second, 1000);
        assert_eq!(config.burst_size, 100);
    }

    #[test]
    fn test_rate_limit_config_builder() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(200)
            .with_bytes_per_second(2048 * 1024)
            .with_operations_per_second(2000)
            .with_burst_size(50);

        assert_eq!(config.messages_per_second, 200);
        assert_eq!(config.bytes_per_second, 2048 * 1024);
        assert_eq!(config.operations_per_second, 2000);
        assert_eq!(config.burst_size, 50);
    }

    #[test]
    fn test_rate_limit_config_disabled() {
        let config = RateLimitConfig::disabled();
        assert_eq!(config.messages_per_second, u64::MAX);
        assert_eq!(config.bytes_per_second, u64::MAX);
        assert_eq!(config.operations_per_second, u64::MAX);
        assert_eq!(config.burst_size, u64::MAX);
    }

    #[test]
    fn test_rate_limiter_try_acquire_within_limit() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(10)
            .with_burst_size(10);

        let limiter = RateLimiter::new(config);

        // Should be able to acquire up to burst size
        for _ in 0..10 {
            assert!(limiter.try_acquire(RateLimitType::Message, 1).is_ok());
        }

        // Next acquisition should fail
        assert!(limiter.try_acquire(RateLimitType::Message, 1).is_err());
    }

    #[test]
    fn test_rate_limiter_try_acquire_amount() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(10)
            .with_burst_size(10);

        let limiter = RateLimiter::new(config);

        // Acquire 5 tokens at once
        assert!(limiter.try_acquire(RateLimitType::Message, 5).is_ok());

        // Should have 5 tokens left
        assert!(limiter.try_acquire(RateLimitType::Message, 5).is_ok());

        // Should be exhausted
        assert!(limiter.try_acquire(RateLimitType::Message, 1).is_err());
    }

    #[test]
    fn test_rate_limiter_zero_amount() {
        let limiter = RateLimiter::default_config();

        // Zero amount should always succeed
        assert!(limiter.try_acquire(RateLimitType::Message, 0).is_ok());
        assert!(limiter.try_acquire(RateLimitType::Byte, 0).is_ok());
        assert!(limiter.try_acquire(RateLimitType::Operation, 0).is_ok());
    }

    #[test]
    fn test_rate_limiter_status() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(10)
            .with_burst_size(10);

        let limiter = RateLimiter::new(config);

        let status = limiter.status(RateLimitType::Message);
        assert_eq!(status.remaining, 10);
        assert_eq!(status.capacity, 10);
        assert_eq!(status.limit, 10);

        // Acquire some tokens
        assert!(limiter.try_acquire(RateLimitType::Message, 3).is_ok());

        let status = limiter.status(RateLimitType::Message);
        assert_eq!(status.remaining, 7);
    }

    #[test]
    fn test_rate_limiter_reset() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(10)
            .with_burst_size(10);

        let limiter = RateLimiter::new(config);

        // Exhaust tokens
        for _ in 0..10 {
            assert!(limiter.try_acquire(RateLimitType::Message, 1).is_ok());
        }
        assert!(limiter.try_acquire(RateLimitType::Message, 1).is_err());

        // Reset
        limiter.reset();

        // Should be able to acquire again
        assert!(limiter.try_acquire(RateLimitType::Message, 1).is_ok());
    }

    #[test]
    fn test_rate_limiter_reset_type() {
        let limiter = RateLimiter::default_config();

        // Exhaust message tokens
        for _ in 0..100 {
            if limiter.try_acquire(RateLimitType::Message, 1).is_err() {
                break;
            }
        }

        // Reset only message type
        limiter.reset_type(RateLimitType::Message);

        // Should be able to acquire messages again
        assert!(limiter.try_acquire(RateLimitType::Message, 1).is_ok());
    }

    #[test]
    fn test_rate_limiter_disabled() {
        let limiter = RateLimiter::disabled();

        // Should be able to acquire unlimited tokens
        for _ in 0..10000 {
            assert!(limiter.try_acquire(RateLimitType::Message, 1).is_ok());
        }

        assert!(!limiter.is_enabled());
    }

    #[test]
    fn test_rate_limiter_multiple_types() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(10)
            .with_bytes_per_second(1024)
            .with_operations_per_second(100)
            .with_burst_size(10);

        let limiter = RateLimiter::new(config);

        // Each type should have independent limits
        assert!(limiter.try_acquire(RateLimitType::Message, 10).is_ok());
        assert!(limiter.try_acquire(RateLimitType::Byte, 10).is_ok());
        assert!(limiter.try_acquire(RateLimitType::Operation, 10).is_ok());

        // Exhaust message limit (burst_size=10)
        assert!(limiter.try_acquire(RateLimitType::Message, 1).is_err());

        // Exhaust operation limit (burst_size=10)
        assert!(limiter.try_acquire(RateLimitType::Operation, 1).is_err());

        // Bytes should still work (capacity is max(10, 1024) = 1024)
        assert!(limiter.try_acquire(RateLimitType::Byte, 1).is_ok());
    }

    #[test]
    fn test_rate_limiter_burst_size_smaller_than_limit() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(100)
            .with_burst_size(10);

        let limiter = RateLimiter::new(config);

        let status = limiter.status(RateLimitType::Message);
        assert_eq!(status.capacity, 10); // Burst size is the capacity
        assert_eq!(status.limit, 100);
    }

    #[test]
    fn test_rate_limiter_burst_size_larger_than_limit() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(10)
            .with_burst_size(100);

        let limiter = RateLimiter::new(config);

        let status = limiter.status(RateLimitType::Message);
        assert_eq!(status.capacity, 100); // Burst size is the capacity
        assert_eq!(status.limit, 10);
    }

    #[test]
    fn test_rate_limiter_is_enabled() {
        let limiter = RateLimiter::default_config();
        assert!(limiter.is_enabled());

        let limiter = RateLimiter::disabled();
        assert!(!limiter.is_enabled());
    }

    #[test]
    fn test_rate_limiter_config() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(50);

        let limiter = RateLimiter::new(config);
        assert_eq!(limiter.config().messages_per_second, 50);
    }

    #[test]
    fn test_global_rate_limiter_connection_count() {
        let limiter = GlobalRateLimiter::default_config(10);

        assert_eq!(limiter.connection_count(), 0);

        assert!(limiter.add_connection().is_ok());
        assert_eq!(limiter.connection_count(), 1);

        assert!(limiter.add_connection().is_ok());
        assert_eq!(limiter.connection_count(), 2);

        limiter.remove_connection();
        assert_eq!(limiter.connection_count(), 1);
    }

    #[test]
    fn test_global_rate_limiter_connection_limit() {
        let limiter = GlobalRateLimiter::default_config(2);

        assert!(limiter.add_connection().is_ok());
        assert!(limiter.add_connection().is_ok());

        // Third connection should fail
        assert!(limiter.add_connection().is_err());

        // Check error type
        match limiter.add_connection() {
            Err(Error::ConnectionLimitExceeded { current, limit }) => {
                assert_eq!(current, 3);
                assert_eq!(limit, 2);
            }
            _ => panic!("Expected ConnectionLimitExceeded error"),
        }
    }

    #[test]
    fn test_global_rate_limiter_delegates_to_limiter() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(10)
            .with_burst_size(10);

        let limiter = GlobalRateLimiter::new(config, 100);

        // Should delegate rate limiting
        for _ in 0..10 {
            assert!(limiter.try_acquire(RateLimitType::Message, 1).is_ok());
        }

        assert!(limiter.try_acquire(RateLimitType::Message, 1).is_err());
    }

    #[test]
    fn test_global_rate_limiter_status() {
        let limiter = GlobalRateLimiter::default_config(100);

        let status = limiter.status(RateLimitType::Message);
        assert!(status.remaining > 0);
    }

    #[test]
    fn test_global_rate_limiter_is_enabled() {
        let limiter = GlobalRateLimiter::default_config(100);
        assert!(limiter.is_enabled());
    }

    #[test]
    fn test_rate_limit_type_all() {
        let types = RateLimitType::all();
        assert_eq!(types.len(), 3);
        assert!(types.contains(&RateLimitType::Message));
        assert!(types.contains(&RateLimitType::Byte));
        assert!(types.contains(&RateLimitType::Operation));
    }

    #[test]
    fn test_token_bucket_refill() {
        let bucket = TokenBucket::new(10, 100); // 10 capacity, 100 per second refill

        // Consume all tokens
        assert!(bucket.try_consume(10));
        assert_eq!(bucket.available(), 0);

        // Should not be able to consume more
        assert!(!bucket.try_consume(1));

        // Wait a bit for refill
        thread::sleep(Duration::from_millis(20));

        // Should have some tokens now
        let available = bucket.available();
        assert!(available > 0, "Expected tokens to refill, got {}", available);
    }

    #[test]
    fn test_token_bucket_time_until_full() {
        let bucket = TokenBucket::new(100, 10); // 100 capacity, 10 per second

        // Consume half
        assert!(bucket.try_consume(50));

        let time_until_full = bucket.time_until_full();
        assert!(time_until_full.as_secs() >= 4);
        assert!(time_until_full.as_secs() <= 6);
    }

    #[test]
    fn test_token_bucket_reset() {
        let bucket = TokenBucket::new(10, 100);

        // Consume all
        assert!(bucket.try_consume(10));
        assert_eq!(bucket.available(), 0);

        // Reset
        bucket.reset();
        assert_eq!(bucket.available(), 10);
    }

    #[test]
    fn test_rate_limit_status_clone() {
        let config = RateLimitConfig::new()
            .with_messages_per_second(10)
            .with_burst_size(10);

        let limiter = RateLimiter::new(config);

        let status1 = limiter.status(RateLimitType::Message);
        let status2 = status1.clone();

        assert_eq!(status1.remaining, status2.remaining);
        assert_eq!(status1.limit, status2.limit);
        assert_eq!(status1.capacity, status2.capacity);
    }

    #[test]
    fn test_rate_limiter_default_trait() {
        let limiter1 = RateLimiter::default();
        let limiter2 = RateLimiter::default_config();

        // Both should have same configuration
        assert_eq!(limiter1.config().messages_per_second, limiter2.config().messages_per_second);
    }

    #[test]
    fn test_rate_limit_error_display() {
        let result: Result<()> = Err(Error::RateLimitExceeded {
            rate_limit_type: RateLimitType::Message,
            retry_after: Duration::from_secs(5),
        });

        assert!(result.unwrap_err().to_string().contains("Rate limit"));
    }

    #[test]
    fn test_concurrent_acquire() {
        let limiter = Arc::new(RateLimiter::new(
            RateLimitConfig::new()
                .with_messages_per_second(1000)
                .with_burst_size(100)
        ));

        let mut handles = vec![];

        // Spawn multiple threads trying to acquire
        for _ in 0..10 {
            let limiter_clone = Arc::clone(&limiter);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    let _ = limiter_clone.try_acquire(RateLimitType::Message, 1);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Should have used all tokens
        let status = limiter.status(RateLimitType::Message);
        assert!(status.remaining < 100);
    }

    #[tokio::test]
    async fn test_async_acquire() {
        let limiter = RateLimiter::new(
            RateLimitConfig::new()
                .with_messages_per_second(100)
                .with_burst_size(5)
        );

        // Exhaust tokens
        for _ in 0..5 {
            assert!(limiter.try_acquire(RateLimitType::Message, 1).is_ok());
        }

        // Should block and eventually succeed after refill
        let start = Instant::now();
        limiter.acquire(RateLimitType::Message, 1).await.unwrap();
        let elapsed = start.elapsed();

        // Should have waited at least a bit for refill
        assert!(elapsed.as_millis() > 10);
    }

    #[test]
    fn test_byte_rate_limiting() {
        let config = RateLimitConfig::new()
            .with_bytes_per_second(1024)
            .with_burst_size(2048);

        let limiter = RateLimiter::new(config);

        // Should be able to send up to burst size
        assert!(limiter.try_acquire(RateLimitType::Byte, 2048).is_ok());

        // Should be exhausted
        assert!(limiter.try_acquire(RateLimitType::Byte, 1).is_err());

        // Status should show bytes
        let status = limiter.status(RateLimitType::Byte);
        assert_eq!(status.remaining, 0);
        assert_eq!(status.capacity, 2048);
    }

    #[test]
    fn test_operation_rate_limiting() {
        let config = RateLimitConfig::new()
            .with_operations_per_second(1000)
            .with_burst_size(500);

        let limiter = RateLimiter::new(config);

        // Should be able to perform up to burst size operations
        for _ in 0..500 {
            assert!(limiter.try_acquire(RateLimitType::Operation, 1).is_ok());
        }

        // Should be exhausted
        assert!(limiter.try_acquire(RateLimitType::Operation, 1).is_err());
    }
}

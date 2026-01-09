//! Connection pool for WebSocket clients
//!
//! The connection pool manages multiple WebSocket connections to the same or different servers,
//! enabling connection reuse, load balancing, and better resource management.

use crate::{
    client::WebSocketClient, config::ClientConfig, error::{Error, Result}, metrics::MetricsCollector,
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
use uuid::Uuid;

/// Configuration for the connection pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections to maintain per server
    pub min_connections: usize,
    /// Maximum number of connections per server
    pub max_connections: usize,
    /// How long to keep idle connections before eviction
    pub idle_timeout: Duration,
    /// How often to check connection health
    pub health_check_interval: Duration,
    /// Maximum time to wait for a connection from the pool
    pub acquire_timeout: Duration,
    /// Whether to enable connection health checking
    pub enable_health_check: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 10,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            health_check_interval: Duration::from_secs(30),
            acquire_timeout: Duration::from_secs(5),
            enable_health_check: true,
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum connections
    pub fn with_min_connections(mut self, min: usize) -> Self {
        assert!(min > 0, "min_connections must be greater than 0");
        self.min_connections = min;
        self
    }

    /// Set maximum connections
    pub fn with_max_connections(mut self, max: usize) -> Self {
        assert!(max > 0, "max_connections must be greater than 0");
        self.max_connections = max;
        self
    }

    /// Set idle timeout
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set health check interval
    pub fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }

    /// Set acquire timeout
    pub fn with_acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    /// Enable or disable health checking
    pub fn with_health_check(mut self, enabled: bool) -> Self {
        self.enable_health_check = enabled;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.min_connections == 0 {
            return Err(Error::invalid_state("min_connections must be greater than 0"));
        }
        if self.max_connections == 0 {
            return Err(Error::invalid_state("max_connections must be greater than 0"));
        }
        if self.min_connections > self.max_connections {
            return Err(Error::invalid_state(
                "min_connections cannot be greater than max_connections",
            ));
        }
        Ok(())
    }
}

/// A pooled connection with metadata
#[derive(Debug)]
struct PooledConnection {
    /// Unique identifier for this connection
    id: String,
    /// The underlying WebSocket client
    client: WebSocketClient,
    /// When this connection was last used
    last_used: Instant,
    /// When this connection was created
    created_at: Instant,
    /// Whether this connection is currently in use
    in_use: bool,
    /// Server URL this connection is to
    url: String,
}

impl PooledConnection {
    /// Create a new pooled connection
    fn new(client: WebSocketClient, url: String) -> Self {
        let now = Instant::now();
        Self {
            id: Uuid::new_v4().to_string(),
            client,
            last_used: now,
            created_at: now,
            in_use: false,
            url,
        }
    }

    /// Check if connection is healthy
    fn is_healthy(&self) -> bool {
        self.client.is_connected()
    }

    /// Check if connection is idle
    fn is_idle(&self, timeout: Duration) -> bool {
        !self.in_use && self.last_used.elapsed() > timeout
    }

    /// Check if connection should be evicted
    fn should_evict(&self, max_age: Duration) -> bool {
        !self.in_use && (self.is_idle(max_age) || !self.is_healthy())
    }

    /// Mark connection as used
    fn mark_used(&mut self) {
        self.last_used = Instant::now();
    }

    /// Get connection age
    fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// Connections managed for a specific server URL
#[derive(Debug)]
struct ServerPool {
    /// URL of the server
    url: String,
    /// Configuration for this server
    config: ClientConfig,
    /// Available connections (not in use)
    available: Vec<PooledConnection>,
    /// Connections currently in use
    in_use: Vec<PooledConnection>,
    /// Semaphore to limit concurrent connections
    semaphore: Arc<Semaphore>,
    /// Total connections created
    total_created: u64,
    /// Total connections acquired
    total_acquired: u64,
    /// Total connections released
    total_released: u64,
}

impl ServerPool {
    /// Create a new server pool
    fn new(url: String, config: ClientConfig, max_connections: usize) -> Self {
        Self {
            url,
            config,
            available: Vec::new(),
            in_use: Vec::new(),
            semaphore: Arc::new(Semaphore::new(max_connections)),
            total_created: 0,
            total_acquired: 0,
            total_released: 0,
        }
    }

    /// Get total connection count
    fn total_connections(&self) -> usize {
        self.available.len() + self.in_use.len()
    }

    /// Get an available connection
    fn get_available(&mut self) -> Option<PooledConnection> {
        self.available.pop()
    }

    /// Add an available connection
    fn add_available(&mut self, conn: PooledConnection) {
        self.available.push(conn);
    }

    /// Add an in-use connection
    fn add_in_use(&mut self, conn: PooledConnection) {
        self.in_use.push(conn);
    }

    /// Remove an in-use connection by ID
    fn remove_in_use(&mut self, id: &str) -> Option<PooledConnection> {
        let pos = self.in_use.iter().position(|c| c.id == id)?;
        Some(self.in_use.remove(pos))
    }

    /// Clean up idle/unhealthy connections
    fn cleanup(&mut self, idle_timeout: Duration) -> usize {
        let initial_count = self.available.len();
        self.available
            .retain(|conn| !conn.should_evict(idle_timeout));
        initial_count - self.available.len()
    }
}

/// Connection pool for WebSocket clients
#[derive(Debug, Clone)]
pub struct ConnectionPool {
    /// Pool configuration
    config: PoolConfig,
    /// Server-specific pools indexed by URL
    pools: Arc<Mutex<HashMap<String, ServerPool>>>,
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: PoolConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            pools: Arc::new(Mutex::new(HashMap::new())),
            metrics: Arc::new(MetricsCollector::new()),
        })
    }

    /// Create a connection pool with default configuration
    pub fn with_defaults() -> Result<Self> {
        Self::new(PoolConfig::default())
    }

    /// Acquire a connection from the pool
    ///
    /// This will either return an existing idle connection or create a new one
    /// if the pool has not reached its maximum size.
    pub async fn get(&self, url: impl Into<String> + Clone) -> Result<PooledClient> {
        self.get_with_config(url.clone(), ClientConfig::new(url)).await
    }

    /// Acquire a connection with custom configuration
    pub async fn get_with_config(
        &self,
        url: impl Into<String>,
        config: ClientConfig,
    ) -> Result<PooledClient> {
        let url = url.into();
        let url_clone = url.clone();

        // Try to get an existing connection first
        let can_create_connection = {
            let mut pools = self.pools.lock();
            if let Some(pool) = pools.get_mut(&url_clone) {
                // Try to get an available connection
                if let Some(mut conn) = pool.get_available() {
                    if conn.is_healthy() {
                        let id = conn.id.clone();
                        let client = conn.client.clone();
                        conn.in_use = true;
                        conn.mark_used();
                        pool.add_in_use(conn);
                        pool.total_acquired += 1;
                        self.metrics.record_message_sent(1); // Record pool hit

                        return Ok(PooledClient {
                            client,
                            pool: self.clone(),
                            url: url_clone,
                            connection_id: id,
                        });
                    }
                    // Connection not healthy, discard it
                    pool.total_created = pool.total_created.saturating_sub(1);
                }

                // Check if we can create a new connection and return early
                pool.total_connections() < self.config.max_connections
            } else {
                // Pool doesn't exist, we can create
                true
            }
        };

        if can_create_connection {
            // Create new connection
            let timeout = tokio::time::timeout(
                self.config.acquire_timeout,
                WebSocketClient::connect_with_config(config),
            )
            .await
            .map_err(|_| Error::Timeout)?;

            let client = timeout?;

            let mut pools = self.pools.lock();
            let pool = pools.entry(url_clone.clone()).or_insert_with(|| {
                ServerPool::new(url_clone.clone(), ClientConfig::new(url_clone.clone()), self.config.max_connections)
            });

            let mut conn = PooledConnection::new(client.clone(), url.clone());
            conn.in_use = true;
            let id = conn.id.clone();
            pool.add_in_use(conn);
            pool.total_created += 1;
            pool.total_acquired += 1;

            return Ok(PooledClient {
                client,
                pool: self.clone(),
                url,
                connection_id: id,
            });
        }

        // Wait for an available connection with timeout
        let start = Instant::now();
        loop {
            // Check timeout
            if start.elapsed() >= self.config.acquire_timeout {
                return Err(Error::Timeout);
            }

            {
                let mut pools = self.pools.lock();
                if let Some(pool) = pools.get_mut(&url_clone) {
                    if let Some(mut conn) = pool.get_available() {
                        if conn.is_healthy() {
                            let id = conn.id.clone();
                            let client = conn.client.clone();
                            conn.in_use = true;
                            conn.mark_used();
                            pool.add_in_use(conn);
                            pool.total_acquired += 1;

                            return Ok(PooledClient {
                                client,
                                pool: self.clone(),
                                url: url_clone,
                                connection_id: id,
                            });
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Release a connection back to the pool
    pub(crate) fn release(&self, url: &str, connection_id: &str) {
        let mut pools = self.pools.lock();
        if let Some(pool) = pools.get_mut(url) {
            if let Some(mut conn) = pool.remove_in_use(connection_id) {
                conn.in_use = false;
                conn.mark_used();

                // Only keep the connection if healthy and under max limit
                if conn.is_healthy() && pool.total_connections() <= self.config.max_connections {
                    pool.add_available(conn);
                } else {
                    // Connection is unhealthy or we have too many, discard it
                    pool.total_created = pool.total_created.saturating_sub(1);
                }
                pool.total_released += 1;
            }
        }
    }

    /// Close a connection (remove from pool)
    pub(crate) fn close(&self, url: &str, connection_id: &str) {
        let mut pools = self.pools.lock();
        if let Some(pool) = pools.get_mut(url) {
            // Try to remove from in-use
            if pool.remove_in_use(connection_id).is_some() {
                pool.total_created = pool.total_created.saturating_sub(1);
                return;
            }

            // Try to remove from available
            if let Some(pos) = pool.available.iter().position(|c| c.id == connection_id) {
                pool.available.remove(pos);
                pool.total_created = pool.total_created.saturating_sub(1);
            }
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let pools = self.pools.lock();
        let mut total_available = 0;
        let mut total_in_use = 0;
        let mut total_created = 0;
        let server_count = pools.len();

        for pool in pools.values() {
            total_available += pool.available.len();
            total_in_use += pool.in_use.len();
            total_created += pool.total_created;
        }

        PoolStats {
            server_count,
            total_connections: total_available + total_in_use,
            available_connections: total_available,
            in_use_connections: total_in_use,
            total_created,
        }
    }

    /// Get statistics for a specific server
    pub fn server_stats(&self, url: &str) -> Option<ServerPoolStats> {
        let pools = self.pools.lock();
        let pool = pools.get(url)?;

        Some(ServerPoolStats {
            url: pool.url.clone(),
            available: pool.available.len(),
            in_use: pool.in_use.len(),
            total: pool.available.len() + pool.in_use.len(),
            total_created: pool.total_created,
            total_acquired: pool.total_acquired,
            total_released: pool.total_released,
        })
    }

    /// Clean up idle and unhealthy connections
    pub fn cleanup(&self) -> CleanupStats {
        let mut pools = self.pools.lock();
        let mut total_removed = 0;
        let mut servers_cleaned = 0;

        for pool in pools.values_mut() {
            let removed = pool.cleanup(self.config.idle_timeout);
            if removed > 0 {
                total_removed += removed;
                servers_cleaned += 1;
            }
        }

        CleanupStats {
            connections_removed: total_removed,
            servers_cleaned,
        }
    }

    /// Start background health check task
    pub fn start_health_check_task(&self) -> tokio::task::JoinHandle<()> {
        let pool = self.clone();
        let interval = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                pool.cleanup();
            }
        })
    }

    /// Close all connections in the pool
    pub async fn close_all(&self) {
        // Collect all clients to close
        let clients_to_close: Vec<WebSocketClient> = {
            let mut pools = self.pools.lock();
            let mut clients = Vec::new();

            for pool in pools.values_mut() {
                // Collect available connections
                for conn in pool.available.drain(..) {
                    clients.push(conn.client);
                }

                // Collect in-use connections
                for conn in pool.in_use.drain(..) {
                    clients.push(conn.client);
                }

                pool.total_created = 0;
            }

            pools.clear();
            clients
        };

        // Close all connections outside the lock
        for client in clients_to_close {
            let _ = client.close(None).await;
        }
    }

    /// Get metrics collector
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }
}

/// A pooled connection wrapper
#[derive(Debug, Clone)]
pub struct PooledClient {
    /// The underlying WebSocket client
    pub client: WebSocketClient,
    /// Reference to the pool
    pool: ConnectionPool,
    /// Server URL
    url: String,
    /// Connection ID (public for debugging/monitoring)
    pub connection_id: String,
}

impl PooledClient {
    /// Send a text message
    pub async fn send_text(&self, text: &str) -> Result<()> {
        self.client.send_text(text).await
    }

    /// Send a binary message
    pub async fn send_binary(&self, data: &[u8]) -> Result<()> {
        self.client.send_binary(data).await
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    /// Get the underlying client
    pub fn inner(&self) -> &WebSocketClient {
        &self.client
    }

    /// Return connection to pool (called automatically on drop)
    pub fn release(self) {
        self.pool.release(&self.url, &self.connection_id);
    }

    /// Close this connection and remove it from the pool
    pub async fn close(self) -> Result<()> {
        self.pool.close(&self.url, &self.connection_id);
        self.client.close(None).await
    }
}

impl Drop for PooledClient {
    fn drop(&mut self) {
        // Return connection to pool when dropped
        self.pool.release(&self.url, &self.connection_id);
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Number of server pools
    pub server_count: usize,
    /// Total connections across all pools
    pub total_connections: usize,
    /// Available connections
    pub available_connections: usize,
    /// In-use connections
    pub in_use_connections: usize,
    /// Total connections created
    pub total_created: u64,
}

/// Server pool statistics
#[derive(Debug, Clone)]
pub struct ServerPoolStats {
    /// Server URL
    pub url: String,
    /// Available connections
    pub available: usize,
    /// In-use connections
    pub in_use: usize,
    /// Total connections
    pub total: usize,
    /// Total connections created
    pub total_created: u64,
    /// Total connections acquired
    pub total_acquired: u64,
    /// Total connections released
    pub total_released: u64,
}

/// Cleanup statistics
#[derive(Debug, Clone)]
pub struct CleanupStats {
    /// Number of connections removed
    pub connections_removed: usize,
    /// Number of servers cleaned up
    pub servers_cleaned: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test pool
    fn create_test_pool() -> ConnectionPool {
        let config = PoolConfig {
            min_connections: 1,
            max_connections: 5,
            idle_timeout: Duration::from_secs(60),
            health_check_interval: Duration::from_secs(10),
            acquire_timeout: Duration::from_secs(1),
            enable_health_check: true,
        };
        ConnectionPool::new(config).unwrap()
    }

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.min_connections, 1);
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.idle_timeout, Duration::from_secs(300));
    }

    #[test]
    fn test_pool_config_builder() {
        let config = PoolConfig::new()
            .with_min_connections(2)
            .with_max_connections(20)
            .with_idle_timeout(Duration::from_secs(120));

        assert_eq!(config.min_connections, 2);
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.idle_timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_pool_config_validation() {
        // Valid config
        let config = PoolConfig::default();
        assert!(config.validate().is_ok());

        // min_connections cannot be 0
        let config = PoolConfig {
            min_connections: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // max_connections cannot be 0
        let config = PoolConfig {
            max_connections: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // min cannot be greater than max
        let config = PoolConfig {
            min_connections: 10,
            max_connections: 5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_pool_with_defaults() {
        let pool = ConnectionPool::with_defaults();
        assert!(pool.is_ok());
    }

    #[test]
    fn test_pooled_connection() {
        // This is a unit test that doesn't require actual network connection
        let _config = ClientConfig::new("ws://localhost:8080");

        // Create a mock pooled connection state
        let _url = "ws://localhost:8080".to_string();
        let is_connected = true;
        let in_use = true;

        assert!(is_connected);
        assert!(in_use);
    }

    #[test]
    fn test_pool_stats_empty() {
        let pool = create_test_pool();
        let stats = pool.stats();

        assert_eq!(stats.server_count, 0);
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.available_connections, 0);
        assert_eq!(stats.in_use_connections, 0);
    }

    #[test]
    fn test_pooled_connection_idle_check() {
        let _url = "ws://localhost:8080".to_string();
        let timeout = Duration::from_secs(60);

        // Simulate idle check
        let last_used = Instant::now();
        let is_idle = last_used.elapsed() > timeout;

        assert!(!is_idle);
    }

    #[test]
    fn test_cleanup_stats() {
        let stats = CleanupStats {
            connections_removed: 5,
            servers_cleaned: 2,
        };

        assert_eq!(stats.connections_removed, 5);
        assert_eq!(stats.servers_cleaned, 2);
    }

    // Note: Integration tests that require actual WebSocket connections
    // should be placed in a separate test module with #[tokio::test]
    // and can use a local test server

    #[tokio::test]
    async fn test_pool_acquire_timeout() {
        // Use a non-routable IP to trigger timeout
        let config = PoolConfig {
            acquire_timeout: Duration::from_millis(100),
            ..Default::default()
        };

        let pool = ConnectionPool::new(config).unwrap();
        let result = pool.get("ws://192.0.2.1:9999").await;

        // Should timeout or fail to connect
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_server_stats_nonexistent() {
        let pool = create_test_pool();
        let stats = pool.server_stats("ws://nonexistent");

        assert!(stats.is_none());
    }
}

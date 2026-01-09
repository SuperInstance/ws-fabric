//! Metrics and telemetry collection for WebSocket connections

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Metrics collector
#[derive(Debug)]
pub struct MetricsCollector {
    metrics: Arc<Mutex<ConnectionMetrics>>,
}

#[derive(Debug, Default)]
struct ConnectionMetrics {
    // Connection metrics
    active_connections: u64,
    total_connections: u64,
    disconnections: u64,
    connection_errors: u64,
    reconnections: u64,

    // Message metrics
    messages_sent: u64,
    messages_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
    send_errors: u64,
    receive_errors: u64,

    // Latency metrics (in microseconds)
    latencies: Vec<u64>,

    // Timing metrics
    #[allow(dead_code)]
    connection_duration: Duration,
    last_activity: Option<Instant>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(ConnectionMetrics::default())),
        }
    }

    /// Record a new connection
    pub fn record_connection(&self) {
        let mut metrics = self.metrics.lock();
        metrics.active_connections += 1;
        metrics.total_connections += 1;
        metrics.last_activity = Some(Instant::now());
    }

    /// Record a disconnection
    pub fn record_disconnection(&self) {
        let mut metrics = self.metrics.lock();
        metrics.active_connections = metrics.active_connections.saturating_sub(1);
        metrics.disconnections += 1;
        metrics.last_activity = Some(Instant::now());
    }

    /// Record a connection error
    pub fn record_connection_error(&self) {
        let mut metrics = self.metrics.lock();
        metrics.connection_errors += 1;
    }

    /// Record a reconnection
    pub fn record_reconnection(&self) {
        let mut metrics = self.metrics.lock();
        metrics.reconnections += 1;
    }

    /// Record a sent message
    pub fn record_message_sent(&self, size: usize) {
        let mut metrics = self.metrics.lock();
        metrics.messages_sent += 1;
        metrics.bytes_sent += size as u64;
        metrics.last_activity = Some(Instant::now());
    }

    /// Record a received message
    pub fn record_message_received(&self, size: usize) {
        let mut metrics = self.metrics.lock();
        metrics.messages_received += 1;
        metrics.bytes_received += size as u64;
        metrics.last_activity = Some(Instant::now());
    }

    /// Record a send error
    pub fn record_send_error(&self) {
        let mut metrics = self.metrics.lock();
        metrics.send_errors += 1;
    }

    /// Record a receive error
    pub fn record_receive_error(&self) {
        let mut metrics = self.metrics.lock();
        metrics.receive_errors += 1;
    }

    /// Record message latency
    pub fn record_latency(&self, latency: Duration) {
        let mut metrics = self.metrics.lock();
        let micros = latency.as_micros() as u64;
        metrics.latencies.push(micros);

        // Keep only last 1000 latency measurements
        if metrics.latencies.len() > 1000 {
            metrics.latencies.remove(0);
        }
    }

    /// Get active connections count
    pub fn active_connections(&self) -> u64 {
        let metrics = self.metrics.lock();
        metrics.active_connections
    }

    /// Get total connections count
    pub fn total_connections(&self) -> u64 {
        let metrics = self.metrics.lock();
        metrics.total_connections
    }

    /// Get message statistics
    pub fn message_stats(&self) -> (u64, u64) {
        let metrics = self.metrics.lock();
        (metrics.messages_sent, metrics.messages_received)
    }

    /// Get byte statistics
    pub fn byte_stats(&self) -> (u64, u64) {
        let metrics = self.metrics.lock();
        (metrics.bytes_sent, metrics.bytes_received)
    }

    /// Get latency percentiles
    pub fn latency_percentiles(&self) -> LatencyPercentiles {
        let metrics = self.metrics.lock();

        if metrics.latencies.is_empty() {
            return LatencyPercentiles {
                p50: 0,
                p95: 0,
                p99: 0,
                p999: 0,
            };
        }

        let mut sorted = metrics.latencies.clone();
        sorted.sort();

        let len = sorted.len();

        LatencyPercentiles {
            p50: sorted[len * 50 / 100],
            p95: sorted[len * 95 / 100],
            p99: sorted[len * 99 / 100],
            p999: sorted[len.saturating_sub(1) * 999 / 1000],
        }
    }

    /// Get last activity time
    pub fn last_activity(&self) -> Option<Instant> {
        let metrics = self.metrics.lock();
        metrics.last_activity
    }

    /// Check if connection is idle (no activity for specified duration)
    pub fn is_idle(&self, timeout: Duration) -> bool {
        if let Some(last) = self.last_activity() {
            last.elapsed() > timeout
        } else {
            false
        }
    }

    /// Get error counts
    pub fn error_counts(&self) -> (u64, u64, u64) {
        let metrics = self.metrics.lock();
        (
            metrics.connection_errors,
            metrics.send_errors,
            metrics.receive_errors,
        )
    }

    /// Get all metrics as a map
    pub fn snapshot(&self) -> HashMap<String, u64> {
        let mut snapshot = HashMap::new();
        let metrics = self.metrics.lock();

        snapshot.insert("active_connections".to_string(), metrics.active_connections);
        snapshot.insert("total_connections".to_string(), metrics.total_connections);
        snapshot.insert("disconnections".to_string(), metrics.disconnections);
        snapshot.insert("connection_errors".to_string(), metrics.connection_errors);
        snapshot.insert("reconnections".to_string(), metrics.reconnections);
        snapshot.insert("messages_sent".to_string(), metrics.messages_sent);
        snapshot.insert("messages_received".to_string(), metrics.messages_received);
        snapshot.insert("bytes_sent".to_string(), metrics.bytes_sent);
        snapshot.insert("bytes_received".to_string(), metrics.bytes_received);
        snapshot.insert("send_errors".to_string(), metrics.send_errors);
        snapshot.insert("receive_errors".to_string(), metrics.receive_errors);

        snapshot
    }

    /// Reset all metrics
    pub fn reset(&self) {
        let mut metrics = self.metrics.lock();
        *metrics = ConnectionMetrics::default();
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Latency percentiles
#[derive(Debug, Clone, Copy)]
pub struct LatencyPercentiles {
    /// 50th percentile (median)
    pub p50: u64,
    /// 95th percentile
    pub p95: u64,
    /// 99th percentile
    pub p99: u64,
    /// 99.9th percentile
    pub p999: u64,
}

impl LatencyPercentiles {
    /// Format latency as string
    pub fn format(&self, percentile: &str) -> String {
        let micros = match percentile {
            "p50" => self.p50,
            "p95" => self.p95,
            "p99" => self.p99,
            "p999" => self.p999,
            _ => self.p50,
        };

        if micros < 1000 {
            format!("{}µs", micros)
        } else if micros < 1_000_000 {
            format!("{:.2}ms", micros as f64 / 1000.0)
        } else {
            format!("{:.2}s", micros as f64 / 1_000_000.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_connection_tracking() {
        let collector = MetricsCollector::new();

        collector.record_connection();
        assert_eq!(collector.active_connections(), 1);
        assert_eq!(collector.total_connections(), 1);

        collector.record_disconnection();
        assert_eq!(collector.active_connections(), 0);
        assert_eq!(collector.total_connections(), 1);
    }

    #[test]
    fn test_metrics_message_tracking() {
        let collector = MetricsCollector::new();

        collector.record_message_sent(100);
        collector.record_message_received(50);

        let (sent, recv) = collector.message_stats();
        assert_eq!(sent, 1);
        assert_eq!(recv, 1);

        let (sent_bytes, recv_bytes) = collector.byte_stats();
        assert_eq!(sent_bytes, 100);
        assert_eq!(recv_bytes, 50);
    }

    #[test]
    fn test_metrics_latency() {
        let collector = MetricsCollector::new();

        collector.record_latency(Duration::from_micros(100));
        collector.record_latency(Duration::from_micros(200));
        collector.record_latency(Duration::from_micros(300));

        let percentiles = collector.latency_percentiles();
        assert_eq!(percentiles.p50, 200);
        assert!(percentiles.p95 >= 200);
        assert!(percentiles.p99 <= 300);
    }

    #[test]
    fn test_metrics_idle_detection() {
        let collector = MetricsCollector::new();

        // No activity yet
        assert!(!collector.is_idle(Duration::from_secs(1)));

        // Record activity
        collector.record_message_sent(100);
        assert!(!collector.is_idle(Duration::from_secs(1)));

        // Manually set last activity to past
        {
            let mut metrics = collector.metrics.lock();
            metrics.last_activity = Some(Instant::now() - Duration::from_secs(10));
        }

        assert!(collector.is_idle(Duration::from_secs(5)));
        assert!(!collector.is_idle(Duration::from_secs(15)));
    }

    #[test]
    fn test_metrics_snapshot() {
        let collector = MetricsCollector::new();

        collector.record_connection();
        collector.record_message_sent(100);

        let snapshot = collector.snapshot();
        assert_eq!(snapshot.get("active_connections"), Some(&1));
        assert_eq!(snapshot.get("messages_sent"), Some(&1));
    }

    #[test]
    fn test_metrics_reset() {
        let collector = MetricsCollector::new();

        collector.record_connection();
        collector.record_message_sent(100);

        collector.reset();

        assert_eq!(collector.active_connections(), 0);
        assert_eq!(collector.total_connections(), 0);
    }

    #[test]
    fn test_latency_formatting() {
        let percentiles = LatencyPercentiles {
            p50: 100,
            p95: 1500,
            p99: 150_000,
            p999: 1_500_000,
        };

        assert_eq!(percentiles.format("p50"), "100µs");
        assert_eq!(percentiles.format("p95"), "1.50ms");
        assert_eq!(percentiles.format("p99"), "150.00ms");
        assert_eq!(percentiles.format("p999"), "1.50s");
    }
}

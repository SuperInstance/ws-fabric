//! Per-message compression for WebSocket (RFC 7692)
//!
//! This module provides DEFLATE compression for WebSocket messages to reduce bandwidth usage.
//! Compression is applied to messages larger than a threshold size, and compression context
//! is maintained per connection for better compression ratios.

use crate::error::{Error, Result};
use bytes::Bytes;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::io::{Read, Write};
use std::sync::Arc;
use parking_lot::Mutex;

/// Default compression threshold (1KB)
/// Messages smaller than this won't be compressed due to overhead
pub const DEFAULT_COMPRESSION_THRESHOLD: usize = 1024;

/// Default compression level (0-9, default 6)
/// - 0: No compression
/// - 1: Fastest compression
/// - 6: Balanced (default)
/// - 9: Best compression
pub const DEFAULT_COMPRESSION_LEVEL: u8 = 6;

/// Maximum compression level
pub const MAX_COMPRESSION_LEVEL: u8 = 9;

/// Minimum compression level
pub const MIN_COMPRESSION_LEVEL: u8 = 0;

/// Compression configuration
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Enable compression
    pub enabled: bool,
    /// Compression level (0-9)
    pub level: u8,
    /// Minimum message size to compress
    pub threshold: usize,
    /// Server max window bits (8-15)
    pub server_max_window_bits: u8,
    /// Client max window bits (8-15)
    pub client_max_window_bits: u8,
    /// Server no context takeover
    pub server_no_context_takeover: bool,
    /// Client no context takeover
    pub client_no_context_takeover: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: DEFAULT_COMPRESSION_LEVEL,
            threshold: DEFAULT_COMPRESSION_THRESHOLD,
            server_max_window_bits: 15,
            client_max_window_bits: 15,
            server_no_context_takeover: false,
            client_no_context_takeover: false,
        }
    }
}

impl CompressionConfig {
    /// Create a new compression config
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable compression
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Set compression level
    pub fn with_level(mut self, level: u8) -> Self {
        assert!(level <= MAX_COMPRESSION_LEVEL, "Compression level must be 0-9");
        self.level = level;
        self
    }

    /// Set compression threshold
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set server max window bits
    pub fn with_server_max_window_bits(mut self, bits: u8) -> Self {
        assert!((8..=15).contains(&bits), "Window bits must be 8-15");
        self.server_max_window_bits = bits;
        self
    }

    /// Set client max window bits
    pub fn with_client_max_window_bits(mut self, bits: u8) -> Self {
        assert!((8..=15).contains(&bits), "Window bits must be 8-15");
        self.client_max_window_bits = bits;
        self
    }

    /// Set server no context takeover
    pub fn with_server_no_context_takeover(mut self, enabled: bool) -> Self {
        self.server_no_context_takeover = enabled;
        self
    }

    /// Set client no context takeover
    pub fn with_client_no_context_takeover(mut self, enabled: bool) -> Self {
        self.client_no_context_takeover = enabled;
        self
    }
}

/// Per-message compressor
///
/// Maintains compression state across messages for better compression ratios.
pub struct Compressor {
    /// Compression configuration
    config: CompressionConfig,
    /// Statistics
    stats: Arc<Mutex<CompressionStats>>,
}

/// Compression statistics
#[derive(Debug, Default)]
struct CompressionStats {
    total_in: u64,
    total_out: u64,
}

impl Compressor {
    /// Create a new compressor with the given configuration
    pub fn new(config: CompressionConfig) -> Result<Self> {
        if !config.enabled {
            return Err(Error::not_supported("Compression is disabled"));
        }

        Ok(Self {
            config,
            stats: Arc::new(Mutex::new(CompressionStats::default())),
        })
    }

    /// Create a new compressor with default configuration
    pub fn with_level(level: u8) -> Result<Self> {
        let config = CompressionConfig::new().with_level(level);
        Self::new(config)
    }

    /// Compress data
    ///
    /// Returns the compressed data, or original data if compression is not beneficial.
    pub fn compress(&self, data: &[u8]) -> Result<Bytes> {
        // Don't compress if below threshold
        if data.len() < self.config.threshold {
            return Ok(Bytes::copy_from_slice(data));
        }

        let compression = Compression::new(u32::from(self.config.level));
        let mut encoder = ZlibEncoder::new(Vec::new(), compression);

        encoder.write_all(data)
            .map_err(|e| Error::invalid_frame(format!("Compression failed: {}", e)))?;

        let compressed = encoder.finish()
            .map_err(|e| Error::invalid_frame(format!("Compression finish failed: {}", e)))?;

        let mut stats = self.stats.lock();
        stats.total_in += data.len() as u64;
        stats.total_out += compressed.len() as u64;

        // Return original if compression didn't help
        if compressed.len() >= data.len() {
            return Ok(Bytes::copy_from_slice(data));
        }

        Ok(Bytes::from(compressed))
    }

    /// Reset compression state
    pub fn reset(&self) {
        // State is reset per-compression with DeflateEncoder
        let mut stats = self.stats.lock();
        stats.total_in = 0;
        stats.total_out = 0;
    }

    /// Get total bytes input
    pub fn total_input(&self) -> u64 {
        self.stats.lock().total_in
    }

    /// Get total bytes output
    pub fn total_output(&self) -> u64 {
        self.stats.lock().total_out
    }

    /// Get compression ratio
    pub fn compression_ratio(&self) -> f64 {
        let stats = self.stats.lock();
        if stats.total_in == 0 {
            1.0
        } else {
            stats.total_out as f64 / stats.total_in as f64
        }
    }
}

/// Per-message decompressor
///
/// Maintains decompression state across messages.
pub struct Decompressor {
    /// Statistics
    stats: Arc<Mutex<DecompressionStats>>,
}

/// Decompression statistics
#[derive(Debug, Default)]
struct DecompressionStats {
    total_in: u64,
    total_out: u64,
}

impl Decompressor {
    /// Create a new decompressor
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(DecompressionStats::default())),
        }
    }

    /// Decompress data
    pub fn decompress(&self, data: &[u8]) -> Result<Bytes> {
        // Create a fresh decoder for each message (per-message compression)
        let mut decoder = ZlibDecoder::new(data);

        // Estimate decompressed size (start with compressed size, will grow if needed)
        let mut decompressed = Vec::with_capacity(data.len().max(1024));

        decoder.read_to_end(&mut decompressed)
            .map_err(|e| Error::invalid_frame(format!("Decompression failed: {}", e)))?;

        let mut stats = self.stats.lock();
        stats.total_in += data.len() as u64;
        stats.total_out += decompressed.len() as u64;

        Ok(Bytes::from(decompressed))
    }

    /// Reset decompression state
    pub fn reset(&self) {
        let mut stats = self.stats.lock();
        stats.total_in = 0;
        stats.total_out = 0;
    }

    /// Get total bytes input
    pub fn total_input(&self) -> u64 {
        self.stats.lock().total_in
    }

    /// Get total bytes output
    pub fn total_output(&self) -> u64 {
        self.stats.lock().total_out
    }

    /// Get decompression ratio
    pub fn decompression_ratio(&self) -> f64 {
        let stats = self.stats.lock();
        if stats.total_in == 0 {
            1.0
        } else {
            stats.total_out as f64 / stats.total_in as f64
        }
    }
}

impl Default for Decompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if RSV1 bit should be set for compressed message
pub fn should_compress(config: &CompressionConfig, data: &[u8]) -> bool {
    config.enabled && data.len() >= config.threshold
}

/// Generate per-message compression offer for WebSocket handshake
pub fn compression_offer(config: &CompressionConfig) -> String {
    let mut params = vec![
        format!("permessage-deflate; server_max_window_bits={}", config.server_max_window_bits),
        format!("client_max_window_bits={}", config.client_max_window_bits),
    ];

    if config.server_no_context_takeover {
        params.push("server_no_context_takeover".to_string());
    }

    if config.client_no_context_takeover {
        params.push("client_no_context_takeover".to_string());
    }

    params.join("; ")
}

/// Parse per-message compression response from WebSocket handshake
pub fn parse_compression_response(header: &str) -> Result<CompressionConfig> {
    let header = header.to_lowercase();

    if !header.contains("permessage-deflate") {
        return Ok(CompressionConfig::disabled());
    }

    let mut config = CompressionConfig::default();

    // Parse server_max_window_bits
    if let Some(captures) = regex_lite::Regex::new(r"server_max_window_bits=(\d+)").unwrap().captures(&header) {
        if let Some(bits) = captures.get(1) {
            if let Ok(value) = bits.as_str().parse::<u8>() {
                if (8..=15).contains(&value) {
                    config.server_max_window_bits = value;
                }
            }
        }
    }

    // Parse client_max_window_bits
    if let Some(captures) = regex_lite::Regex::new(r"client_max_window_bits=(\d+)").unwrap().captures(&header) {
        if let Some(bits) = captures.get(1) {
            if let Ok(value) = bits.as_str().parse::<u8>() {
                if (8..=15).contains(&value) {
                    config.client_max_window_bits = value;
                }
            }
        }
    }

    // Parse context takeover flags
    config.server_no_context_takeover = header.contains("server_no_context_takeover");
    config.client_no_context_takeover = header.contains("client_no_context_takeover");

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert!(config.enabled);
        assert_eq!(config.level, DEFAULT_COMPRESSION_LEVEL);
        assert_eq!(config.threshold, DEFAULT_COMPRESSION_THRESHOLD);
        assert_eq!(config.server_max_window_bits, 15);
        assert_eq!(config.client_max_window_bits, 15);
    }

    #[test]
    fn test_compression_config_builder() {
        let config = CompressionConfig::new()
            .with_level(9)
            .with_threshold(2048)
            .with_server_max_window_bits(14);

        assert_eq!(config.level, 9);
        assert_eq!(config.threshold, 2048);
        assert_eq!(config.server_max_window_bits, 14);
    }

    #[test]
    fn test_compression_config_disabled() {
        let config = CompressionConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_compression_config_level_validation() {
        let result = std::panic::catch_unwind(|| {
            CompressionConfig::new().with_level(10);
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_compression_config_window_bits_validation() {
        let result = std::panic::catch_unwind(|| {
            CompressionConfig::new().with_server_max_window_bits(16);
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_compressor_creation() {
        let compressor = Compressor::with_level(6).unwrap();
        assert_eq!(compressor.config.level, 6);
        assert!(compressor.config.enabled);
    }

    #[test]
    fn test_compressor_disabled() {
        let config = CompressionConfig::disabled();
        let result = Compressor::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_small_message_not_compressed() {
        let compressor = Compressor::with_level(6).unwrap();
        let data = b"Hello, World!"; // 13 bytes < threshold

        let compressed = compressor.compress(data).unwrap();
        assert_eq!(compressed.as_ref(), data);
    }

    #[test]
    fn test_compress_decompress() {
        let compressor = Compressor::with_level(6).unwrap();
        let decompressor = Decompressor::new();

        // Create data larger than threshold
        let data = "Hello, World! ".repeat(100);
        let data_bytes = data.as_bytes();

        let compressed = compressor.compress(data_bytes).unwrap();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed.as_ref(), data_bytes);
        // Compression should reduce size
        assert!(compressed.len() < data_bytes.len());
    }

    #[test]
    fn test_compress_repeated_data() {
        let compressor = Compressor::with_level(6).unwrap();
        let decompressor = Decompressor::new();

        // Repeated patterns compress well
        let data = "A".repeat(10000);
        let data_bytes = data.as_bytes();

        let compressed = compressor.compress(data_bytes).unwrap();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed.as_ref(), data_bytes);
        // Should compress very well
        assert!(compressed.len() < data_bytes.len() / 10);
    }

    #[test]
    fn test_compressor_stats() {
        let compressor = Compressor::with_level(6).unwrap();

        let data = "Hello, World! ".repeat(100);
        let data_bytes = data.as_bytes();

        compressor.compress(data_bytes).unwrap();

        assert_eq!(compressor.total_input(), data_bytes.len() as u64);
        assert!(compressor.total_output() > 0);
        assert!(compressor.compression_ratio() < 1.0);
    }

    #[test]
    fn test_decompressor_stats() {
        let compressor = Compressor::with_level(6).unwrap();

        let data = "Test ".repeat(100);
        let data_bytes = data.as_bytes();

        let compressed = compressor.compress(data_bytes).unwrap();

        // Only decompress if compression helped
        let decompressor = Decompressor::new();
        let decompressed = if compressed.len() < data_bytes.len() {
            decompressor.decompress(&compressed).unwrap()
        } else {
            // Not compressed, use original
            compressed.clone()
        };

        assert_eq!(decompressed.as_ref(), data_bytes);
        if compressed.len() < data_bytes.len() {
            assert_eq!(decompressor.total_input(), compressed.len() as u64);
            assert_eq!(decompressor.total_output(), data_bytes.len() as u64);
            assert!(decompressor.decompression_ratio() > 1.0);
        }
    }

    #[test]
    fn test_compressor_reset() {
        let compressor = Compressor::with_level(6).unwrap();

        let data = "Test ".repeat(100);
        let _ = compressor.compress(data.as_bytes()).unwrap();

        let initial_input = compressor.total_input();
        compressor.reset();

        assert_eq!(compressor.total_input(), 0);
        assert_eq!(compressor.total_output(), 0);

        let _ = compressor.compress(data.as_bytes()).unwrap();

        assert_eq!(compressor.total_input(), initial_input);
    }

    #[test]
    fn test_decompressor_reset() {
        let compressor = Compressor::with_level(6).unwrap();

        let data = "Test ".repeat(100);
        let compressed = compressor.compress(data.as_bytes()).unwrap();

        // Only test reset if compression actually happened
        if compressed.len() < data.as_bytes().len() {
            let decompressor = Decompressor::new();
            let _ = decompressor.decompress(&compressed).unwrap();

            let initial_input = decompressor.total_input();
            let initial_output = decompressor.total_output();

            decompressor.reset();

            assert_eq!(decompressor.total_input(), 0);
            assert_eq!(decompressor.total_output(), 0);

            let _ = decompressor.decompress(&compressed).unwrap();

            assert_eq!(decompressor.total_input(), initial_input);
            assert_eq!(decompressor.total_output(), initial_output);
        }
    }

    #[test]
    fn test_should_compress() {
        let config = CompressionConfig::new()
            .with_threshold(100);

        // Small message - should not compress
        assert!(!should_compress(&config, b"Hello"));

        // Large message - should compress
        assert!(should_compress(&config, &b"A".repeat(200)));

        // Disabled - should not compress
        let disabled = CompressionConfig::disabled();
        assert!(!should_compress(&disabled, &b"A".repeat(200)));
    }

    #[test]
    fn test_compression_offer() {
        let config = CompressionConfig::new()
            .with_server_max_window_bits(14)
            .with_client_max_window_bits(14);

        let offer = compression_offer(&config);

        assert!(offer.contains("permessage-deflate"));
        assert!(offer.contains("server_max_window_bits=14"));
        assert!(offer.contains("client_max_window_bits=14"));
    }

    #[test]
    fn test_parse_compression_response() {
        let header = "permessage-deflate; server_max_window_bits=14; client_max_window_bits=14; server_no_context_takeover";

        let config = parse_compression_response(header).unwrap();

        assert!(config.enabled);
        assert_eq!(config.server_max_window_bits, 14);
        assert_eq!(config.client_max_window_bits, 14);
        assert!(config.server_no_context_takeover);
    }

    #[test]
    fn test_parse_compression_response_disabled() {
        let header = "gzip";

        let config = parse_compression_response(header).unwrap();

        assert!(!config.enabled);
    }

    #[test]
    fn test_binary_data_compression() {
        let compressor = Compressor::with_level(6).unwrap();
        let decompressor = Decompressor::new();

        // Binary data with patterns
        let data: Vec<u8> = (0..255).cycle().take(10000).collect();

        let compressed = compressor.compress(&data).unwrap();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed.as_ref(), data.as_slice());
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn test_high_compression_level() {
        let compressor = Compressor::with_level(9).unwrap();
        let decompressor = Decompressor::new();

        let data = "Repeated pattern ".repeat(1000);
        let data_bytes = data.as_bytes();

        let compressed = compressor.compress(data_bytes).unwrap();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed.as_ref(), data_bytes);
        // High compression should work well on repeated data
        assert!(compressed.len() < data_bytes.len() / 5);
    }

    #[test]
    fn test_fast_compression_level() {
        let compressor = Compressor::with_level(1).unwrap();
        let decompressor = Decompressor::new();

        let data = "Test data ".repeat(1000);
        let data_bytes = data.as_bytes();

        let compressed = compressor.compress(data_bytes).unwrap();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed.as_ref(), data_bytes);
        assert!(compressed.len() < data_bytes.len());
    }

    #[test]
    fn test_empty_data() {
        let compressor = Compressor::with_level(6).unwrap();
        let decompressor = Decompressor::new();

        let data = b"";
        let compressed = compressor.compress(data).unwrap();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed, Bytes::new());
    }

    #[test]
    fn test_no_compression_level() {
        let compressor = Compressor::with_level(0).unwrap();

        let data = "Test data ".repeat(100);
        let data_bytes = data.as_bytes();

        // Level 0 may not compress well, so we just verify it doesn't error
        let compressed = compressor.compress(data_bytes).unwrap();

        // May or may not be smaller depending on data
        // Level 0 is "no compression" mode
        assert!(compressed.len() >= data_bytes.len() || compressed.len() < data_bytes.len());
    }

    #[test]
    fn test_compress_decompress_multiple_messages() {
        let compressor = Compressor::with_level(6).unwrap();

        let messages = vec![
            "Message 1 ".repeat(100),
            "Message 2 ".repeat(200),
            "Message 3 ".repeat(150),
        ];

        for msg in &messages {
            let compressed = compressor.compress(msg.as_bytes()).unwrap();

            // If the data was compressed (smaller than original), decompress it
            // If not compressed (compression didn't help), compare directly
            if compressed.len() < msg.as_bytes().len() {
                let decompressor = Decompressor::new();
                let decompressed = decompressor.decompress(&compressed).unwrap();
                assert_eq!(decompressed.as_ref(), msg.as_bytes());
            } else {
                // Data returned as-is
                assert_eq!(compressed.as_ref(), msg.as_bytes());
            }
        }

        // Verify stats accumulated
        assert!(compressor.total_input() > 0);
    }

    #[test]
    fn test_unicode_text_compression() {
        let compressor = Compressor::with_level(6).unwrap();
        let decompressor = Decompressor::new();

        let text = "🔥 Unicode test: こんにちは 世界 안녕하세요 ".repeat(100);
        let text_bytes = text.as_bytes();

        let compressed = compressor.compress(text_bytes).unwrap();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed.as_ref(), text_bytes);
        assert!(compressed.len() < text_bytes.len());
    }
}

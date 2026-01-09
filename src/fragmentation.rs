//! WebSocket message fragmentation and reassembly
//!
//! This module implements RFC 6455 message fragmentation, allowing large messages
//! to be split across multiple frames and reassembled on the receiving end.
//!
//! # Fragmentation
//!
//! The [`Fragmenter`] splits large messages into multiple frames:
//! - First frame: carries the actual opcode (text 0x1 or binary 0x2)
//! - Middle frames: use continuation opcode (0x0)
//! - Final frame: has FIN bit set, uses continuation opcode
//!
//! # Reassembly
//!
//! The [`Reassembler`] collects fragmented frames and reconstructs complete messages:
//! - Tracks partial messages by connection ID
//! - Times out incomplete reassemblies
//! - Handles interleaved control frames (ping, pong, close)
//!
//! # Example
//!
//! ```rust
//! use websocket_fabric::fragmentation::{Fragmenter, Reassembler};
//! use websocket_fabric::Message;
//! use std::time::Duration;
//!
//! // Fragment a large message
//! let fragmenter = Fragmenter::new(16 * 1024); // 16KB max frame size
//! let message = Message::text(vec!["A"; 50000].join(""));
//! let frames = fragmenter.split_message(&message);
//!
//! // Reassemble on receiving end
//! let mut reassembler = Reassembler::new(Duration::from_secs(5));
//! for frame in frames {
//!     if let Some(complete) = reassembler.ingest_frame(&frame, 1)? {
//!         println!("Reassembled: {}", complete.as_text().unwrap());
//!     }
//! }
//! ```

use crate::error::{Error, Result};
use crate::message::{Frame, Message, MessageType};
use bytes::Bytes;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Default maximum frame size (16KB)
pub const DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024;

/// Default reassembly timeout (5 seconds)
pub const DEFAULT_REASSEMBLY_TIMEOUT: Duration = Duration::from_secs(5);

/// WebSocket opcodes (RFC 6455)
const OPCODE_CONTINUATION: u8 = 0x0;
const OPCODE_TEXT: u8 = 0x1;
const OPCODE_BINARY: u8 = 0x2;
const OPCODE_CLOSE: u8 = 0x8;
const OPCODE_PING: u8 = 0x9;
const OPCODE_PONG: u8 = 0xA;

/// Connection identifier for tracking fragmented messages
pub type ConnectionId = u64;

/// Message fragmenter that splits large messages into multiple frames
///
/// The fragmenter ensures no frame exceeds the specified maximum size by
/// splitting messages across multiple frames according to RFC 6455.
#[derive(Debug, Clone)]
pub struct Fragmenter {
    /// Maximum payload size per frame
    max_frame_size: usize,
}

impl Fragmenter {
    /// Create a new fragmenter with the specified maximum frame size
    ///
    /// # Arguments
    ///
    /// * `max_frame_size` - Maximum payload bytes per frame (must be >= 125)
    ///
    /// # Panics
    ///
    /// Panics if `max_frame_size` is less than 125 (minimum for valid WebSocket frames)
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::fragmentation::Fragmenter;
    ///
    /// let fragmenter = Fragmenter::new(16 * 1024); // 16KB frames
    /// ```
    pub fn new(max_frame_size: usize) -> Self {
        assert!(
            max_frame_size >= 125,
            "max_frame_size must be at least 125 bytes"
        );
        Self { max_frame_size }
    }

    /// Get the maximum frame size
    pub fn max_frame_size(&self) -> usize {
        self.max_frame_size
    }

    /// Split a message into multiple frames if necessary
    ///
    /// # Arguments
    ///
    /// * `message` - The message to split
    ///
    /// # Returns
    ///
    /// A vector of frames. If the message fits in one frame, returns a single
    /// frame with FIN=1. Otherwise, returns multiple frames with FIN=0 on all
    /// but the last.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::fragmentation::Fragmenter;
    /// use websocket_fabric::Message;
    ///
    /// let fragmenter = Fragmenter::new(100);
    /// let message = Message::text("Hello, World!");
    /// let frames = fragmenter.split_message(&message);
    ///
    /// assert!(!frames.is_empty());
    /// assert!(frames.last().unwrap().fin); // Last frame has FIN=1
    /// ```
    pub fn split_message(&self, message: &Message) -> Vec<Frame> {
        let payload = &message.payload;
        let payload_len = payload.len();

        // If message fits in one frame, return it unfragmented
        if payload_len <= self.max_frame_size {
            let opcode = match message.msg_type {
                MessageType::Text => OPCODE_TEXT,
                MessageType::Binary => OPCODE_BINARY,
                MessageType::Ping => OPCODE_PING,
                MessageType::Pong => OPCODE_PONG,
                MessageType::Close => OPCODE_CLOSE,
                MessageType::Continuation => OPCODE_CONTINUATION,
            };

            return vec![Frame::new(true, opcode, payload.clone())];
        }

        // Split into multiple frames
        let mut frames = Vec::new();
        let chunk_count = payload_len.div_ceil(self.max_frame_size);

        for (i, chunk) in payload.chunks(self.max_frame_size).enumerate() {
            let is_first = i == 0;
            let is_last = i == chunk_count - 1;

            let opcode = if is_first {
                match message.msg_type {
                    MessageType::Text => OPCODE_TEXT,
                    MessageType::Binary => OPCODE_BINARY,
                    _ => OPCODE_BINARY, // Default to binary for other types
                }
            } else {
                OPCODE_CONTINUATION
            };

            let fin = is_last;
            frames.push(Frame::new(fin, opcode, Bytes::copy_from_slice(chunk)));
        }

        frames
    }

    /// Calculate number of frames needed for a message size
    ///
    /// # Arguments
    ///
    /// * `message_size` - Size of the message in bytes
    ///
    /// # Returns
    ///
    /// The number of frames required to send a message of this size
    pub fn calculate_frame_count(&self, message_size: usize) -> usize {
        if message_size <= self.max_frame_size {
            1
        } else {
            message_size.div_ceil(self.max_frame_size)
        }
    }

    /// Check if a message needs fragmentation
    ///
    /// # Arguments
    ///
    /// * `message` - The message to check
    ///
    /// # Returns
    ///
    /// `true` if the message exceeds the maximum frame size
    pub fn needs_fragmentation(&self, message: &Message) -> bool {
        message.len() > self.max_frame_size
    }
}

impl Default for Fragmenter {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_FRAME_SIZE)
    }
}

/// A partially reassembled message
#[derive(Debug)]
struct PartialMessage {
    /// Original message opcode (text or binary)
    opcode: u8,
    /// Received fragments in order
    fragments: Vec<Bytes>,
    /// When this partial message was first received
    received_at: Instant,
    /// Total size of all fragments received so far
    total_size: usize,
}

impl PartialMessage {
    /// Create a new partial message
    fn new(opcode: u8, first_fragment: Bytes) -> Self {
        let fragment_len = first_fragment.len();
        Self {
            opcode,
            fragments: vec![first_fragment],
            received_at: Instant::now(),
            total_size: fragment_len,
        }
    }

    /// Add a continuation fragment
    fn add_fragment(&mut self, fragment: Bytes) {
        self.total_size += fragment.len();
        self.fragments.push(fragment);
    }

    /// Check if this partial message has timed out
    fn is_expired(&self, timeout: Duration) -> bool {
        self.received_at.elapsed() > timeout
    }

    /// Get the total reassembled size
    fn total_size(&self) -> usize {
        self.total_size
    }

    /// Reassemble all fragments into a complete message
    fn assemble(self) -> Message {
        let msg_type = match self.opcode {
            OPCODE_TEXT => MessageType::Text,
            _ => MessageType::Binary,
        };

        // Calculate total size
        let total_size = self.fragments.iter().map(|f| f.len()).sum();

        // Allocate and copy all fragments
        let mut payload = Vec::with_capacity(total_size);
        for fragment in self.fragments {
            payload.extend_from_slice(&fragment);
        }

        Message {
            msg_type,
            payload: Bytes::from(payload),
        }
    }
}

/// Message reassembler that collects fragmented frames
///
/// The reassembler tracks partial messages per connection and times out
/// incomplete reassemblies to prevent memory exhaustion.
#[derive(Debug, Clone)]
pub struct Reassembler {
    /// Partial messages being reassembled, keyed by connection ID
    incomplete: Arc<Mutex<HashMap<ConnectionId, PartialMessage>>>,
    /// Timeout for incomplete reassemblies
    timeout: Duration,
}

impl Reassembler {
    /// Create a new reassembler with the specified timeout
    ///
    /// # Arguments
    ///
    /// * `timeout` - How long to wait before discarding incomplete reassemblies
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::fragmentation::Reassembler;
    /// use std::time::Duration;
    ///
    /// let reassembler = Reassembler::new(Duration::from_secs(5));
    /// ```
    pub fn new(timeout: Duration) -> Self {
        Self {
            incomplete: Arc::new(Mutex::new(HashMap::new())),
            timeout,
        }
    }

    /// Get the reassembly timeout
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Ingest a frame and potentially return a complete message
    ///
    /// # Arguments
    ///
    /// * `frame` - The received frame
    /// * `connection_id` - Identifier for the connection receiving this frame
    ///
    /// # Returns
    ///
    /// - `Ok(Some(message))` - A complete message was reassembled
    /// - `Ok(None)` - More frames needed, or this was a control frame
    /// - `Err(Error)` - Protocol violation or timeout
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A continuation frame is received without a starting frame
    /// - A fragmented message times out
    /// - Frame sequence is invalid
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::fragmentation::Reassembler;
    /// use websocket_fabric::message::Frame;
    ///
    /// let mut reassembler = Reassembler::default();
    ///
    /// // Receive frames...
    /// while let Some(frame) = receive_frame() {
    ///     match reassembler.ingest_frame(&frame, 1) {
    ///         Ok(Some(message)) => println!("Complete: {:?}", message),
    ///         Ok(None) => continue, // Need more frames
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// ```
    pub fn ingest_frame(
        &self,
        frame: &Frame,
        connection_id: ConnectionId,
    ) -> Result<Option<Message>> {
        // Handle control frames (never fragmented)
        if Self::is_control_frame(frame.opcode) {
            return Ok(Some(Self::frame_to_message(frame)?));
        }

        let is_fin = frame.fin;
        let opcode = frame.opcode;

        // Clean up expired partial messages
        self.cleanup_expired(connection_id);

        let mut incomplete = self.incomplete.lock();

        if opcode == OPCODE_CONTINUATION {
            // Continuation frame - must have a partial message in progress
            let partial = incomplete
                .get_mut(&connection_id)
                .ok_or_else(|| {
                    Error::protocol_violation(
                        "Continuation frame received without starting frame",
                    )
                })?;

            // Add this fragment
            partial.add_fragment(frame.payload.clone());

            if is_fin {
                // Final frame - reassemble complete message
                let partial = incomplete
                    .remove(&connection_id)
                    .expect("Partial message just existed");

                Ok(Some(partial.assemble()))
            } else {
                // More fragments expected
                Ok(None)
            }
        } else if opcode == OPCODE_TEXT || opcode == OPCODE_BINARY {
            // Starting a new fragmented message
            if incomplete.contains_key(&connection_id) {
                return Err(Error::protocol_violation(
                    "New fragmented message started before previous completed",
                ));
            }

            if is_fin {
                // Not fragmented, return as-is
                Ok(Some(Self::frame_to_message(frame)?))
            } else {
                // First fragment of a fragmented message
                let partial = PartialMessage::new(opcode, frame.payload.clone());
                incomplete.insert(connection_id, partial);
                Ok(None)
            }
        } else {
            // Unknown opcode
            Err(Error::invalid_frame(format!("Unknown opcode: 0x{:02X}", opcode)))
        }
    }

    /// Abort reassembly for a specific connection
    ///
    /// This is useful when a connection is closed and any partial
    /// messages should be discarded.
    ///
    /// # Arguments
    ///
    /// * `connection_id` - The connection to abort reassembly for
    ///
    /// # Returns
    ///
    /// `true` if a partial message was aborted, `false` if none existed
    pub fn abort(&self, connection_id: ConnectionId) -> bool {
        let mut incomplete = self.incomplete.lock();
        incomplete.remove(&connection_id).is_some()
    }

    /// Get the number of partial messages currently being reassembled
    pub fn in_progress_count(&self) -> usize {
        let incomplete = self.incomplete.lock();
        incomplete.len()
    }

    /// Get the total size of all partial messages being tracked
    pub fn in_progress_size(&self) -> usize {
        let incomplete = self.incomplete.lock();
        incomplete.values().map(|p| p.total_size()).sum()
    }

    /// Clean up expired partial messages for a specific connection
    fn cleanup_expired(&self, connection_id: ConnectionId) {
        let mut incomplete = self.incomplete.lock();

        // Check if this connection's partial message is expired
        if let Some(partial) = incomplete.get(&connection_id) {
            if partial.is_expired(self.timeout) {
                incomplete.remove(&connection_id);
            }
        }
    }

    /// Clean up all expired partial messages across all connections
    pub fn cleanup_all_expired(&self) {
        let mut incomplete = self.incomplete.lock();
        incomplete.retain(|_, partial| !partial.is_expired(self.timeout));
    }

    /// Check if an opcode is for a control frame
    fn is_control_frame(opcode: u8) -> bool {
        opcode == OPCODE_CLOSE || opcode == OPCODE_PING || opcode == OPCODE_PONG
    }

    /// Convert a frame to a message
    fn frame_to_message(frame: &Frame) -> Result<Message> {
        let msg_type = match frame.opcode {
            OPCODE_TEXT => MessageType::Text,
            OPCODE_BINARY => MessageType::Binary,
            OPCODE_CLOSE => MessageType::Close,
            OPCODE_PING => MessageType::Ping,
            OPCODE_PONG => MessageType::Pong,
            _ => {
                return Err(Error::invalid_frame(format!(
                    "Unknown opcode: 0x{:02X}",
                    frame.opcode
                )))
            }
        };

        Ok(Message {
            msg_type,
            payload: frame.payload.clone(),
        })
    }
}

impl Default for Reassembler {
    fn default() -> Self {
        Self::new(DEFAULT_REASSEMBLY_TIMEOUT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_fragmenter_creation() {
        let fragmenter = Fragmenter::new(1024);
        assert_eq!(fragmenter.max_frame_size(), 1024);
    }

    #[test]
    fn test_fragmenter_default() {
        let fragmenter = Fragmenter::default();
        assert_eq!(fragmenter.max_frame_size(), DEFAULT_MAX_FRAME_SIZE);
    }

    #[test]
    #[should_panic(expected = "max_frame_size must be at least 125")]
    fn test_fragmenter_too_small() {
        Fragmenter::new(100);
    }

    #[test]
    fn test_fragmenter_small_message() {
        let fragmenter = Fragmenter::new(1024);
        let message = Message::text("Hello, World!");

        let frames = fragmenter.split_message(&message);

        assert_eq!(frames.len(), 1);
        assert!(frames[0].fin);
        assert_eq!(frames[0].opcode, OPCODE_TEXT);
        assert_eq!(&frames[0].payload[..], b"Hello, World!");
    }

    #[test]
    fn test_fragmenter_large_text_message() {
        let fragmenter = Fragmenter::new(200);
        let message = Message::text("A".repeat(500));

        let frames = fragmenter.split_message(&message);

        assert!(frames.len() > 1);
        assert_eq!(frames[0].opcode, OPCODE_TEXT);
        assert!(!frames[0].fin);

        // Middle frames should be continuation
        for frame in &frames[1..frames.len() - 1] {
            assert_eq!(frame.opcode, OPCODE_CONTINUATION);
            assert!(!frame.fin);
        }

        // Last frame should be FIN
        assert!(frames.last().unwrap().fin);
    }

    #[test]
    fn test_fragmenter_large_binary_message() {
        let fragmenter = Fragmenter::new(200);
        let data = vec![0u8; 500];
        let message = Message::binary(data.clone());

        let frames = fragmenter.split_message(&message);

        assert!(frames.len() > 1);
        assert_eq!(frames[0].opcode, OPCODE_BINARY);
        assert!(!frames[0].fin);

        // Middle frames should be continuation
        for frame in &frames[1..frames.len() - 1] {
            assert_eq!(frame.opcode, OPCODE_CONTINUATION);
            assert!(!frame.fin);
        }

        // Last frame should be FIN
        assert!(frames.last().unwrap().fin);
    }

    #[test]
    fn test_fragmenter_exactly_max_size() {
        let fragmenter = Fragmenter::new(200);
        let message = Message::text("A".repeat(200));

        let frames = fragmenter.split_message(&message);

        assert_eq!(frames.len(), 1);
        assert!(frames[0].fin);
    }

    #[test]
    fn test_fragmenter_one_byte_over_max() {
        let fragmenter = Fragmenter::new(200);
        let message = Message::text("A".repeat(201));

        let frames = fragmenter.split_message(&message);

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].payload.len(), 200);
        assert_eq!(frames[1].payload.len(), 1);
    }

    #[test]
    fn test_fragmenter_calculate_frame_count() {
        let fragmenter = Fragmenter::new(200);

        assert_eq!(fragmenter.calculate_frame_count(50), 1);
        assert_eq!(fragmenter.calculate_frame_count(200), 1);
        assert_eq!(fragmenter.calculate_frame_count(201), 2);
        assert_eq!(fragmenter.calculate_frame_count(1000), 5);
        assert_eq!(fragmenter.calculate_frame_count(2000), 10);
    }

    #[test]
    fn test_fragmenter_needs_fragmentation() {
        let fragmenter = Fragmenter::new(200);

        let small_msg = Message::text("small");
        assert!(!fragmenter.needs_fragmentation(&small_msg));

        let large_msg = Message::text("A".repeat(300));
        assert!(fragmenter.needs_fragmentation(&large_msg));
    }

    #[test]
    fn test_reassembler_creation() {
        let reassembler = Reassembler::new(Duration::from_secs(10));
        assert_eq!(reassembler.timeout(), Duration::from_secs(10));
    }

    #[test]
    fn test_reassembler_default() {
        let reassembler = Reassembler::default();
        assert_eq!(reassembler.timeout(), DEFAULT_REASSEMBLY_TIMEOUT);
    }

    #[test]
    fn test_reassembler_unfragmented_message() {
        let reassembler = Reassembler::default();
        let frame = Frame::new(true, OPCODE_TEXT, Bytes::copy_from_slice(b"Hello"));

        let result = reassembler.ingest_frame(&frame, 1);

        assert!(result.is_ok());
        let message = result.unwrap().unwrap();
        assert!(message.is_text());
        assert_eq!(message.as_text().unwrap(), "Hello");
    }

    #[test]
    fn test_reassembler_fragmented_message() {
        let fragmenter = Fragmenter::new(150);
        let reassembler = Reassembler::default();

        let message = Message::text("A".repeat(300));
        let frames = fragmenter.split_message(&message);

        let mut complete_message = None;

        for frame in frames {
            let result = reassembler.ingest_frame(&frame, 1).unwrap();
            if let Some(msg) = result {
                complete_message = Some(msg);
            }
        }

        assert!(complete_message.is_some());
        let reassembled = complete_message.unwrap();
        assert!(reassembled.is_text());
        assert_eq!(reassembled.len(), 300);
        assert_eq!(reassembled.as_text().unwrap(), "A".repeat(300));
    }

    #[test]
    fn test_reassembler_continuation_without_start() {
        let reassembler = Reassembler::default();
        let frame = Frame::new(true, OPCODE_CONTINUATION, Bytes::new());

        let result = reassembler.ingest_frame(&frame, 1);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::ProtocolViolation(_)
        ));
    }

    #[test]
    fn test_reassembler_overlapping_fragments() {
        let reassembler = Reassembler::default();

        // Start first fragmented message
        let frame1 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"First"));
        let result1 = reassembler.ingest_frame(&frame1, 1).unwrap();
        assert!(result1.is_none());

        // Try to start another fragmented message on same connection
        let frame2 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"Second"));
        let result2 = reassembler.ingest_frame(&frame2, 1);

        assert!(result2.is_err());
    }

    #[test]
    fn test_reassembler_multiple_connections() {
        let fragmenter = Fragmenter::new(150);
        let reassembler = Reassembler::default();

        // Create fragmented messages for different connections
        let msg1 = Message::text("A".repeat(300));
        let frames1 = fragmenter.split_message(&msg1);

        let msg2 = Message::text("B".repeat(300));
        let frames2 = fragmenter.split_message(&msg2);

        // Interleave frames from different connections
        let mut results = Vec::new();

        for (i, (frame1, frame2)) in frames1.iter().zip(frames2.iter()).enumerate() {
            if i < frames1.len() {
                if let Some(msg) = reassembler.ingest_frame(frame1, 1).unwrap() {
                    results.push((1, msg));
                }
            }
            if i < frames2.len() {
                if let Some(msg) = reassembler.ingest_frame(frame2, 2).unwrap() {
                    results.push((2, msg));
                }
            }
        }

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1);
        assert_eq!(results[1].0, 2);
    }

    #[test]
    fn test_reassembler_control_frames() {
        let reassembler = Reassembler::default();

        // Ping frame
        let ping = Frame::new(true, OPCODE_PING, Bytes::copy_from_slice(b"ping"));
        let result = reassembler.ingest_frame(&ping, 1).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().is_ping());

        // Pong frame
        let pong = Frame::new(true, OPCODE_PONG, Bytes::copy_from_slice(b"pong"));
        let result = reassembler.ingest_frame(&pong, 1).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().is_pong());

        // Close frame
        let close = Frame::new(true, OPCODE_CLOSE, Bytes::new());
        let result = reassembler.ingest_frame(&close, 1).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().is_close());
    }

    #[test]
    fn test_reassembler_in_progress_count() {
        let reassembler = Reassembler::default();

        assert_eq!(reassembler.in_progress_count(), 0);

        // Start a fragmented message
        let frame1 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"Start"));
        reassembler.ingest_frame(&frame1, 1).unwrap();

        assert_eq!(reassembler.in_progress_count(), 1);

        // Start another on different connection
        let frame2 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"Start2"));
        reassembler.ingest_frame(&frame2, 2).unwrap();

        assert_eq!(reassembler.in_progress_count(), 2);

        // Complete first
        let frame3 = Frame::new(true, OPCODE_CONTINUATION, Bytes::new());
        reassembler.ingest_frame(&frame3, 1).unwrap();

        assert_eq!(reassembler.in_progress_count(), 1);
    }

    #[test]
    fn test_reassembler_abort() {
        let reassembler = Reassembler::default();

        // Start a fragmented message
        let frame1 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"Start"));
        reassembler.ingest_frame(&frame1, 1).unwrap();

        assert_eq!(reassembler.in_progress_count(), 1);

        // Abort it
        let aborted = reassembler.abort(1);
        assert!(aborted);
        assert_eq!(reassembler.in_progress_count(), 0);

        // Abort again returns false
        let aborted = reassembler.abort(1);
        assert!(!aborted);
    }

    #[test]
    fn test_reassembler_timeout() {
        let reassembler = Reassembler::new(Duration::from_millis(100));

        // Start a fragmented message
        let frame1 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"Start"));
        reassembler.ingest_frame(&frame1, 1).unwrap();

        assert_eq!(reassembler.in_progress_count(), 1);

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Try to continue - should fail
        let frame2 = Frame::new(true, OPCODE_CONTINUATION, Bytes::new());
        let result = reassembler.ingest_frame(&frame2, 1);

        assert!(result.is_err());
    }

    #[test]
    fn test_reassembler_cleanup_expired() {
        let reassembler = Reassembler::new(Duration::from_millis(100));

        // Start a fragmented message
        let frame1 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"Start"));
        reassembler.ingest_frame(&frame1, 1).unwrap();

        assert_eq!(reassembler.in_progress_count(), 1);

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Clean up expired
        reassembler.cleanup_all_expired();

        assert_eq!(reassembler.in_progress_count(), 0);
    }

    #[test]
    fn test_reassembler_in_progress_size() {
        let reassembler = Reassembler::default();

        // Start fragmented messages
        let frame1 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"12345"));
        reassembler.ingest_frame(&frame1, 1).unwrap();

        let frame2 = Frame::new(false, OPCODE_TEXT, Bytes::copy_from_slice(b"67890"));
        reassembler.ingest_frame(&frame2, 2).unwrap();

        let size = reassembler.in_progress_size();
        assert_eq!(size, 10);
    }

    #[test]
    fn test_fragmentation_roundtrip_binary() {
        let fragmenter = Fragmenter::new(200);
        let reassembler = Reassembler::default();

        let original_data: Vec<u8> = (0..255).cycle().take(500).collect();
        let original = Message::binary(original_data.clone());

        let frames = fragmenter.split_message(&original);

        let mut reassembled = None;
        for frame in frames {
            if let Some(msg) = reassembler.ingest_frame(&frame, 1).unwrap() {
                reassembled = Some(msg);
            }
        }

        assert!(reassembled.is_some());
        let result = reassembled.unwrap();
        assert!(result.is_binary());
        assert_eq!(result.as_bytes(), &original_data[..]);
    }

    #[test]
    fn test_fragmentation_roundtrip_text() {
        let fragmenter = Fragmenter::new(200);
        let reassembler = Reassembler::default();

        let original_text = "Hello, World! ".repeat(50);
        let original = Message::text(original_text.clone());

        let frames = fragmenter.split_message(&original);

        let mut reassembled = None;
        for frame in frames {
            if let Some(msg) = reassembler.ingest_frame(&frame, 1).unwrap() {
                reassembled = Some(msg);
            }
        }

        assert!(reassembled.is_some());
        let result = reassembled.unwrap();
        assert!(result.is_text());
        assert_eq!(result.as_text().unwrap(), original_text);
    }

    #[test]
    fn test_fragmentation_empty_message() {
        let fragmenter = Fragmenter::new(200);
        let message = Message::text("");

        let frames = fragmenter.split_message(&message);

        assert_eq!(frames.len(), 1);
        assert!(frames[0].fin);
        assert_eq!(frames[0].payload.len(), 0);
    }

    #[test]
    fn test_fragmentation_single_byte() {
        let fragmenter = Fragmenter::new(200);
        let message = Message::text("A");

        let frames = fragmenter.split_message(&message);

        assert_eq!(frames.len(), 1);
        assert!(frames[0].fin);
        assert_eq!(frames[0].payload.len(), 1);
    }

    #[test]
    fn test_unknown_opcode() {
        let reassembler = Reassembler::default();
        let frame = Frame::new(true, 0x7, Bytes::new()); // Invalid opcode

        let result = reassembler.ingest_frame(&frame, 1);

        assert!(result.is_err());
    }

    #[test]
    fn test_reassembler_large_fragmented_binary() {
        let fragmenter = Fragmenter::new(16 * 1024); // 16KB
        let reassembler = Reassembler::default();

        // Create 1MB binary message
        let large_data: Vec<u8> = (0..255).cycle().take(1024 * 1024).collect();
        let original = Message::binary(large_data.clone());

        let frames = fragmenter.split_message(&original);

        // Should be roughly 64 frames (1MB / 16KB)
        assert!(frames.len() > 60 && frames.len() < 70);

        let mut reassembled = None;
        for frame in frames {
            if let Some(msg) = reassembler.ingest_frame(&frame, 1).unwrap() {
                reassembled = Some(msg);
            }
        }

        assert!(reassembled.is_some());
        let result = reassembled.unwrap();
        assert_eq!(result.as_bytes().len(), 1024 * 1024);
        assert_eq!(result.as_bytes(), &large_data[..]);
    }

    #[test]
    fn test_partial_message_assemble() {
        // Create a partial message
        let mut partial = PartialMessage::new(OPCODE_TEXT, Bytes::from("Hello"));
        partial.add_fragment(Bytes::from(", "));
        partial.add_fragment(Bytes::from("World"));

        let message = partial.assemble();

        assert!(message.is_text());
        assert_eq!(message.as_text().unwrap(), "Hello, World");
    }

    #[test]
    fn test_partial_message_is_expired() {
        let partial = PartialMessage::new(OPCODE_TEXT, Bytes::from("Test"));

        // Should not be expired immediately
        assert!(!partial.is_expired(Duration::from_secs(1)));

        // Should be expired after waiting
        thread::sleep(Duration::from_millis(100));
        assert!(partial.is_expired(Duration::from_millis(50)));
    }

    #[test]
    fn test_partial_message_total_size() {
        let mut partial = PartialMessage::new(OPCODE_TEXT, Bytes::from("Hello"));

        assert_eq!(partial.total_size(), 5);

        partial.add_fragment(Bytes::from("World"));

        assert_eq!(partial.total_size(), 10);
    }
}

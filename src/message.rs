//! WebSocket message types and framing

use crate::error::{Error, Result};
use bytes::Bytes;
use std::fmt;

/// WebSocket message type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageType {
    /// Text message (UTF-8)
    Text,
    /// Binary message
    Binary,
    /// Ping message
    Ping,
    /// Pong message
    Pong,
    /// Close message
    Close,
    /// Continuation frame
    Continuation,
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageType::Text => write!(f, "text"),
            MessageType::Binary => write!(f, "binary"),
            MessageType::Ping => write!(f, "ping"),
            MessageType::Pong => write!(f, "pong"),
            MessageType::Close => write!(f, "close"),
            MessageType::Continuation => write!(f, "continuation"),
        }
    }
}

/// WebSocket message
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Message type
    pub msg_type: MessageType,
    /// Message payload
    pub payload: Bytes,
}

impl Message {
    /// Create a new text message
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            msg_type: MessageType::Text,
            payload: Bytes::from(text.into()),
        }
    }

    /// Create a new binary message
    pub fn binary(data: impl Into<Bytes>) -> Self {
        Self {
            msg_type: MessageType::Binary,
            payload: data.into(),
        }
    }

    /// Create a binary message from a slice (copies data)
    pub fn binary_from_slice(data: &[u8]) -> Self {
        Self {
            msg_type: MessageType::Binary,
            payload: Bytes::copy_from_slice(data),
        }
    }

    /// Create a new ping message
    pub fn ping(data: impl Into<Bytes>) -> Self {
        Self {
            msg_type: MessageType::Ping,
            payload: data.into(),
        }
    }

    /// Create a new pong message
    pub fn pong(data: impl Into<Bytes>) -> Self {
        Self {
            msg_type: MessageType::Pong,
            payload: data.into(),
        }
    }

    /// Create a new close message
    pub fn close(code: Option<u16>, reason: Option<String>) -> Self {
        let mut payload = Vec::new();

        if let Some(code) = code {
            payload.extend_from_slice(&code.to_be_bytes());

            if let Some(reason) = reason {
                payload.extend_from_slice(reason.as_bytes());
            }
        }

        Self {
            msg_type: MessageType::Close,
            payload: Bytes::from(payload),
        }
    }

    /// Get message as text (returns error if not text message)
    pub fn as_text(&self) -> Result<String> {
        if self.msg_type != MessageType::Text {
            return Err(Error::invalid_frame(format!(
                "Expected text message, got {}",
                self.msg_type
            )));
        }

        String::from_utf8(self.payload.to_vec())
            .map_err(Error::Utf8Error)
    }

    /// Get message as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.payload
    }

    /// Get message payload
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }

    /// Get message type
    pub fn msg_type(&self) -> &MessageType {
        &self.msg_type
    }

    /// Check if message is text
    pub fn is_text(&self) -> bool {
        self.msg_type == MessageType::Text
    }

    /// Check if message is binary
    pub fn is_binary(&self) -> bool {
        self.msg_type == MessageType::Binary
    }

    /// Check if message is ping
    pub fn is_ping(&self) -> bool {
        self.msg_type == MessageType::Ping
    }

    /// Check if message is pong
    pub fn is_pong(&self) -> bool {
        self.msg_type == MessageType::Pong
    }

    /// Check if message is close
    pub fn is_close(&self) -> bool {
        self.msg_type == MessageType::Close
    }

    /// Create JSON message
    pub fn json<T: serde::Serialize>(value: &T) -> Result<Self> {
        let json = serde_json::to_string(value)
            .map_err(|e| Error::invalid_frame(format!("JSON serialization failed: {}", e)))?;

        Ok(Self::text(json))
    }

    /// Parse JSON from message
    pub fn parse_json<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        if self.msg_type != MessageType::Text {
            return Err(Error::invalid_frame(format!(
                "Cannot parse {} as JSON",
                self.msg_type
            )));
        }

        serde_json::from_slice(&self.payload)
            .map_err(|e| Error::invalid_frame(format!("JSON deserialization failed: {}", e)))
    }

    /// Get message size in bytes
    pub fn len(&self) -> usize {
        self.payload.len()
    }

    /// Check if message is empty
    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} bytes)", self.msg_type, self.len())?;

        if self.is_text() {
            if let Ok(text) = self.as_text() {
                let preview = if text.len() > 50 {
                    format!("{}...", &text[..50])
                } else {
                    text
                };
                write!(f, ": {}", preview)?;
            }
        }

        Ok(())
    }
}

impl From<String> for Message {
    fn from(text: String) -> Self {
        Self::text(text)
    }
}

impl From<&str> for Message {
    fn from(text: &str) -> Self {
        Self::text(text)
    }
}

impl From<Vec<u8>> for Message {
    fn from(data: Vec<u8>) -> Self {
        Self::binary(data)
    }
}

impl From<&[u8]> for Message {
    fn from(data: &[u8]) -> Self {
        Self::binary_from_slice(data)
    }
}

/// WebSocket frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Final frame flag
    pub fin: bool,
    /// Reserved bits
    pub rsv1: bool,
    pub rsv2: bool,
    pub rsv3: bool,
    /// Message opcode
    pub opcode: u8,
    /// Payload data
    pub payload: Bytes,
}

impl Frame {
    /// Create a new frame
    pub fn new(fin: bool, opcode: u8, payload: impl Into<Bytes>) -> Self {
        Self {
            fin,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode,
            payload: payload.into(),
        }
    }

    /// Get frame size
    pub fn len(&self) -> usize {
        self.payload.len()
    }

    /// Check if frame is empty
    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }

    /// Check if frame is final
    pub fn is_final(&self) -> bool {
        self.fin
    }

    /// Get opcode
    pub fn opcode(&self) -> u8 {
        self.opcode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_text() {
        let msg = Message::text("Hello");
        assert!(msg.is_text());
        assert_eq!(msg.as_text().unwrap(), "Hello");
        assert_eq!(msg.len(), 5);
    }

    #[test]
    fn test_message_binary() {
        let data = vec![1, 2, 3, 4];
        let msg = Message::binary(data.clone());
        assert!(msg.is_binary());
        assert_eq!(msg.as_bytes(), &data[..]);
    }

    #[test]
    fn test_message_ping() {
        let msg = Message::ping(&b"ping"[..]);
        assert!(msg.is_ping());
        assert_eq!(msg.as_bytes(), b"ping");
    }

    #[test]
    fn test_message_pong() {
        let msg = Message::pong(&b"pong"[..]);
        assert!(msg.is_pong());
        assert_eq!(msg.as_bytes(), b"pong");
    }

    #[test]
    fn test_message_close() {
        let msg = Message::close(Some(1000), Some("Normal closure".to_string()));
        assert!(msg.is_close());
        assert!(msg.payload.len() > 2); // code + reason
    }

    #[test]
    fn test_message_from_string() {
        let msg = Message::from("Hello");
        assert!(msg.is_text());
        assert_eq!(msg.as_text().unwrap(), "Hello");
    }

    #[test]
    fn test_message_json() {
        #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
        struct Test {
            value: i32,
        }

        let test = Test { value: 42 };
        let msg = Message::json(&test).unwrap();
        assert!(msg.is_text());

        let parsed: Test = msg.parse_json().unwrap();
        assert_eq!(parsed, test);
    }

    #[test]
    fn test_frame_creation() {
        let frame = Frame::new(true, 0x01, Bytes::new());
        assert!(frame.is_final());
        assert_eq!(frame.opcode(), 0x01);
        assert_eq!(frame.len(), 0);
    }
}

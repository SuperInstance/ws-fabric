//! WebSocket subprotocol negotiation (RFC 6455 Section 1.9)
//!
//! This module provides support for subprotocol negotiation during the WebSocket handshake.
//! Clients can request specific subprotocols, and servers select the best match.
//!
//! # Example
//!
//! ```rust
//! use websocket_fabric::subprotocol::{SubprotocolList, SubprotocolNegotiator};
//!
//! // Client-side: Request subprotocols in order of preference
//! let client_protocols = SubprotocolList::new()
//!     .with_protocol("chat-v2")
//!     .with_protocol("chat-v1")
//!     .with_protocol("mqtt");
//!
//! // Generate header for handshake
//! let header = client_protocols.to_header_value();
//! assert_eq!(header, "chat-v2, chat-v1, mqtt");
//!
//! // Server-side: Configure supported subprotocols
//! let server_protocols = SubprotocolNegotiator::new()
//!     .with_protocol("mqtt")
//!     .with_protocol("chat-v2")
//!     .with_protocol("chat-v1");
//!
//! // Negotiate best match
//! let selected = server_protocols.negotiate(&client_protocols);
//! assert_eq!(selected, Some("chat-v2".to_string()));
//!
//! // Parse server response
//! let negotiated = client_protocols.parse_response(Some("chat-v2"))?;
//! assert_eq!(negotiated, Some("chat-v2".to_string()));
//! # Ok::<(), websocket_fabric::error::Error>(())
//! ```

use crate::error::{Error, Result};
use std::fmt;

/// Client-side subprotocol list for protocol negotiation
///
/// Maintains an ordered list of subprotocols the client supports,
/// in order of preference (highest priority first).
///
/// # RFC 6455 Compliance
///
/// - Subprotocol names must be non-empty
/// - Valid characters: alphanumeric, hyphen, underscore
/// - Names are case-sensitive
/// - Order represents preference priority
#[derive(Debug, Clone, Default)]
pub struct SubprotocolList {
    /// Ordered list of subprotocol names (highest priority first)
    protocols: Vec<String>,
}

impl SubprotocolList {
    /// Create an empty subprotocol list
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::new();
    /// assert!(list.is_empty());
    /// assert_eq!(list.len(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            protocols: Vec::new(),
        }
    }

    /// Create a subprotocol list from a slice of strings
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::from_slice(&["chat-v2", "mqtt"]);
    /// assert_eq!(list.len(), 2);
    /// ```
    pub fn from_slice(protocols: &[&str]) -> Result<Self> {
        let mut list = Self::new();
        for protocol in protocols {
            list = list.with_protocol(protocol)?;
        }
        Ok(list)
    }

    /// Add a subprotocol to the list (in order of priority)
    ///
    /// Validates the protocol name according to RFC 6455 rules.
    ///
    /// # Errors
    ///
    /// Returns an error if the protocol name is invalid.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::new()
    ///     .with_protocol("chat-v2")
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    /// ```
    pub fn with_protocol(mut self, protocol: &str) -> Result<Self> {
        Self::validate_protocol(protocol)?;
        self.protocols.push(protocol.to_string());
        Ok(self)
    }

    /// Check if the list is empty
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::new();
    /// assert!(list.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.protocols.is_empty()
    }

    /// Get the number of subprotocols in the list
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::new()
    ///     .with_protocol("chat-v2")
    ///     .unwrap();
    /// assert_eq!(list.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.protocols.len()
    }

    /// Get all protocols in order
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::new()
    ///     .with_protocol("chat-v2")
    ///     .unwrap()
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    ///
    /// let protocols = list.protocols();
    /// assert_eq!(protocols, &["chat-v2", "mqtt"]);
    /// ```
    pub fn protocols(&self) -> &[String] {
        &self.protocols
    }

    /// Check if a specific protocol is in the list
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::new()
    ///     .with_protocol("chat-v2")
    ///     .unwrap()
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    ///
    /// assert!(list.contains("chat-v2"));
    /// assert!(!list.contains("graphql"));
    /// ```
    pub fn contains(&self, protocol: &str) -> bool {
        self.protocols.iter().any(|p| p == protocol)
    }

    /// Generate the Sec-WebSocket-Protocol header value
    ///
    /// Returns a comma-separated list of protocols in order of preference.
    /// Returns an empty string if no protocols are configured.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::new()
    ///     .with_protocol("chat-v2")
    ///     .unwrap()
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    ///
    /// assert_eq!(list.to_header_value(), "chat-v2, mqtt");
    /// ```
    pub fn to_header_value(&self) -> String {
        self.protocols.join(", ")
    }

    /// Parse the server's subprotocol response
    ///
    /// Validates that the server selected a protocol from the client's list.
    /// Returns None if the server didn't select a protocol.
    ///
    /// # Errors
    ///
    /// Returns an error if the server selected a protocol not offered by the client.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let list = SubprotocolList::new()
    ///     .with_protocol("chat-v2")
    ///     .unwrap()
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    ///
    /// // Server selects a protocol from our list
    /// assert_eq!(
    ///     list.parse_response(Some("chat-v2")),
    ///     Ok(Some("chat-v2".to_string()))
    /// );
    ///
    /// // Server selects no protocol
    /// assert_eq!(list.parse_response(None), Ok(None));
    ///
    /// // Server selects a protocol we didn't offer (error)
    /// assert!(list.parse_response(Some("graphql")).is_err());
    /// ```
    pub fn parse_response(&self, selected: Option<&str>) -> Result<Option<String>> {
        match selected {
            None => Ok(None),
            Some(protocol) => {
                if !self.contains(protocol) {
                    return Err(Error::SubprotocolNegotiationFailed(format!(
                        "Server selected protocol '{}' not offered by client",
                        protocol
                    )));
                }
                Ok(Some(protocol.to_string()))
            }
        }
    }

    /// Validate a subprotocol name according to RFC 6455
    ///
    /// # Rules
    ///
    /// - Must be non-empty
    /// - Must contain only alphanumeric characters, hyphens, and underscores
    ///
    /// # Errors
    ///
    /// Returns an error if the protocol name is invalid.
    fn validate_protocol(protocol: &str) -> Result<()> {
        if protocol.is_empty() {
            return Err(Error::InvalidSubprotocol(
                "Subprotocol name cannot be empty".to_string(),
            ));
        }

        if !protocol
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(Error::InvalidSubprotocol(format!(
                "Subprotocol name '{}' contains invalid characters (only alphanumeric, hyphen, underscore allowed)",
                protocol
            )));
        }

        Ok(())
    }
}

impl fmt::Display for SubprotocolList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_header_value())
    }
}

/// Server-side subprotocol negotiator
///
/// Maintains a list of supported subprotocols and selects the best match
/// from a client's offer list.
///
/// # Selection Strategy
///
/// The negotiator selects the first protocol from the client's list that
/// the server also supports. This implements priority-based selection where
/// the client's preference order is respected.
///
/// # Example
///
/// ```rust
/// use websocket_fabric::subprotocol::{SubprotocolList, SubprotocolNegotiator};
///
/// // Server supports these protocols
/// let negotiator = SubprotocolNegotiator::new()
///     .with_protocol("mqtt")
///     .with_protocol("chat-v2")
///     .with_protocol("chat-v1");
///
/// // Client offers these protocols (in preference order)
/// let client_protocols = SubprotocolList::new()
///     .with_protocol("chat-v2")
///     .unwrap()
///     .with_protocol("graphql")
///     .unwrap()
///     .with_protocol("mqtt")
///     .unwrap();
///
/// // Server selects "chat-v2" (first match in client's preference order)
/// let selected = negotiator.negotiate(&client_protocols);
/// assert_eq!(selected, Some("chat-v2".to_string()));
/// ```
#[derive(Debug, Clone, Default)]
pub struct SubprotocolNegotiator {
    /// Set of supported subprotocols
    protocols: Vec<String>,
    /// Whether to support wildcard fallback (accept any protocol)
    allow_wildcard: bool,
}

impl SubprotocolNegotiator {
    /// Create a new subprotocol negotiator
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolNegotiator;
    ///
    /// let negotiator = SubprotocolNegotiator::new();
    /// assert!(negotiator.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            protocols: Vec::new(),
            allow_wildcard: false,
        }
    }

    /// Create a negotiator from a slice of supported protocols
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolNegotiator;
    ///
    /// let negotiator = SubprotocolNegotiator::from_slice(&["mqtt", "chat-v2"]).unwrap();
    /// assert_eq!(negotiator.len(), 2);
    /// ```
    pub fn from_slice(protocols: &[&str]) -> Result<Self> {
        let mut negotiator = Self::new();
        for protocol in protocols {
            negotiator = negotiator.with_protocol(protocol)?;
        }
        Ok(negotiator)
    }

    /// Add a supported subprotocol
    ///
    /// # Errors
    ///
    /// Returns an error if the protocol name is invalid.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolNegotiator;
    ///
    /// let negotiator = SubprotocolNegotiator::new()
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    /// ```
    pub fn with_protocol(mut self, protocol: &str) -> Result<Self> {
        SubprotocolList::validate_protocol(protocol)?;
        if !self.protocols.contains(&protocol.to_string()) {
            self.protocols.push(protocol.to_string());
        }
        Ok(self)
    }

    /// Enable wildcard support (accept any protocol)
    ///
    /// When enabled, the negotiator will accept any protocol from the client's list,
    /// even if it's not explicitly supported. This is useful for proxy servers.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::{SubprotocolList, SubprotocolNegotiator};
    ///
    /// let negotiator = SubprotocolNegotiator::new()
    ///     .with_wildcard(true);
    ///
    /// let client_protocols = SubprotocolList::new()
    ///     .with_protocol("any-protocol")
    ///     .unwrap();
    ///
    /// // With wildcard, accepts any protocol
    /// let selected = negotiator.negotiate(&client_protocols);
    /// assert_eq!(selected, Some("any-protocol".to_string()));
    /// ```
    pub fn with_wildcard(mut self, allow: bool) -> Self {
        self.allow_wildcard = allow;
        self
    }

    /// Set whether to reject connections when no protocol match is found
    ///
    /// This is a convenience method that creates a new negotiator configured
    /// to reject on no match. Note that rejection logic must be implemented
    /// by the server using `should_reject()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::{SubprotocolList, SubprotocolNegotiator};
    ///
    /// let negotiator = SubprotocolNegotiator::new()
    ///     .with_protocol("mqtt")
    ///     .unwrap()
    ///     .with_reject_on_no_match(true);
    ///
    /// let client_protocols = SubprotocolList::new()
    ///     .with_protocol("graphql")
    ///     .unwrap();
    ///
    /// // No match, should reject
    /// assert!(negotiator.should_reject(&client_protocols));
    /// ```
    pub fn with_reject_on_no_match(self, _reject: bool) -> Self {
        // This is a no-op in the negotiator itself
        // The rejection logic is implemented in `should_reject()`
        // This method exists for API compatibility and documentation
        self
    }

    /// Check if the negotiator is empty (supports no protocols)
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolNegotiator;
    ///
    /// let negotiator = SubprotocolNegotiator::new();
    /// assert!(negotiator.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.protocols.is_empty()
    }

    /// Get the number of supported protocols
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolNegotiator;
    ///
    /// let negotiator = SubprotocolNegotiator::new()
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    /// assert_eq!(negotiator.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.protocols.len()
    }

    /// Get all supported protocols
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolNegotiator;
    ///
    /// let negotiator = SubprotocolNegotiator::new()
    ///     .with_protocol("mqtt")
    ///     .unwrap()
    ///     .with_protocol("chat-v2")
    ///     .unwrap();
    ///
    /// let protocols = negotiator.protocols();
    /// assert_eq!(protocols, &["mqtt".to_string(), "chat-v2".to_string()]);
    /// ```
    pub fn protocols(&self) -> &[String] {
        &self.protocols
    }

    /// Check if a specific protocol is supported
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolNegotiator;
    ///
    /// let negotiator = SubprotocolNegotiator::new()
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    ///
    /// assert!(negotiator.supports("mqtt"));
    /// assert!(!negotiator.supports("graphql"));
    /// ```
    pub fn supports(&self, protocol: &str) -> bool {
        self.protocols.iter().any(|p| p == protocol)
    }

    /// Negotiate the best matching subprotocol from a client's offer
    ///
    /// Returns the first protocol from the client's list that the server supports.
    /// Returns None if:
    /// - The client offered no protocols
    /// - No matching protocol was found
    /// - The server supports no protocols (and wildcard is disabled)
    ///
    /// # Selection Algorithm
    ///
    /// 1. If client offers no protocols, return None
    /// 2. If server has wildcard enabled, return client's first preference
    /// 3. Iterate through client's protocols in order
    /// 4. Return first match found
    /// 5. Return None if no match
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::{SubprotocolList, SubprotocolNegotiator};
    ///
    /// let negotiator = SubprotocolNegotiator::new()
    ///     .with_protocol("mqtt")
    ///     .unwrap()
    ///     .with_protocol("chat-v2")
    ///     .unwrap();
    ///
    /// let client_protocols = SubprotocolList::new()
    ///     .with_protocol("chat-v2")
    ///     .unwrap()
    ///     .with_protocol("graphql")
    ///     .unwrap()
    ///     .with_protocol("mqtt")
    ///     .unwrap();
    ///
    /// // Selects "chat-v2" (first match in client's preference order)
    /// let selected = negotiator.negotiate(&client_protocols);
    /// assert_eq!(selected, Some("chat-v2".to_string()));
    /// ```
    pub fn negotiate(&self, client_protocols: &SubprotocolList) -> Option<String> {
        // If client offers no protocols, no negotiation needed
        if client_protocols.is_empty() {
            return None;
        }

        // If wildcard enabled, accept client's first preference
        if self.allow_wildcard && !client_protocols.is_empty() {
            return client_protocols.protocols.first().cloned();
        }

        // Find first matching protocol (client preference order)
        for protocol in client_protocols.protocols() {
            if self.supports(protocol) {
                return Some(protocol.clone());
            }
        }

        // No match found
        None
    }

    /// Check if negotiation should result in connection rejection
    ///
    /// Some servers prefer to reject connections if no protocol match is found,
    /// rather than accepting without a subprotocol.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::{SubprotocolList, SubprotocolNegotiator};
    ///
    /// let negotiator = SubprotocolNegotiator::new()
    ///     .with_protocol("mqtt")
    ///     .unwrap()
    ///     .with_reject_on_no_match(true);
    ///
    /// let client_protocols = SubprotocolList::new()
    ///     .with_protocol("graphql")
    ///     .unwrap();
    ///
    /// // No match, should reject
    /// assert!(negotiator.should_reject(&client_protocols));
    /// ```
    pub fn should_reject(&self, client_protocols: &SubprotocolList) -> bool {
        // Reject if client offered protocols but no match found
        !client_protocols.is_empty() && self.negotiate(client_protocols).is_none()
    }

    /// Parse Sec-WebSocket-Protocol header from client handshake
    ///
    /// Parses a comma-separated list of protocols from the header value.
    /// Returns an empty list if the header is not present or empty.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::subprotocol::SubprotocolList;
    ///
    /// let protocols = SubprotocolList::parse_header("chat-v2, mqtt, graphql");
    /// assert_eq!(protocols.protocols(), &["chat-v2", "mqtt", "graphql"]);
    ///
    /// let empty = SubprotocolList::parse_header("");
    /// assert!(empty.is_empty());
    /// ```
    pub fn parse_header(header_value: &str) -> SubprotocolList {
        let mut protocols = Vec::new();

        for protocol in header_value.split(',') {
            let protocol = protocol.trim();
            if !protocol.is_empty() {
                // Validate and collect valid protocols
                if SubprotocolList::validate_protocol(protocol).is_ok() {
                    protocols.push(protocol.to_string());
                }
            }
        }

        // Build the list directly
        let mut list = SubprotocolList::new();
        for protocol in protocols {
            list.protocols.push(protocol);
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== SubprotocolList Tests =====

    #[test]
    fn test_subprotocol_list_new() {
        let list = SubprotocolList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_subprotocol_list_with_protocol() {
        let list = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap();

        assert_eq!(list.len(), 2);
        assert_eq!(list.protocols(), &["chat-v2".to_string(), "mqtt".to_string()]);
    }

    #[test]
    fn test_subprotocol_list_from_slice() {
        let list = SubprotocolList::from_slice(&["chat-v2", "mqtt", "graphql"]).unwrap();

        assert_eq!(list.len(), 3);
        assert_eq!(
            list.protocols(),
            &["chat-v2".to_string(), "mqtt".to_string(), "graphql".to_string()]
        );
    }

    #[test]
    fn test_subprotocol_list_contains() {
        let list = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap();

        assert!(list.contains("chat-v2"));
        assert!(list.contains("mqtt"));
        assert!(!list.contains("graphql"));
    }

    #[test]
    fn test_subprotocol_list_to_header_value() {
        let list = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap()
            .with_protocol("graphql")
            .unwrap();

        assert_eq!(list.to_header_value(), "chat-v2, mqtt, graphql");
    }

    #[test]
    fn test_subprotocol_list_to_header_value_empty() {
        let list = SubprotocolList::new();
        assert_eq!(list.to_header_value(), "");
    }

    #[test]
    fn test_subprotocol_list_parse_response_success() {
        let list = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap();

        let result = list.parse_response(Some("chat-v2"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("chat-v2".to_string()));
    }

    #[test]
    fn test_subprotocol_list_parse_response_none() {
        let list = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap();

        let result = list.parse_response(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_subprotocol_list_parse_response_invalid() {
        let list = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap();

        let result = list.parse_response(Some("graphql"));
        assert!(result.is_err());
    }

    #[test]
    fn test_subprotocol_list_validate_valid() {
        assert!(SubprotocolList::validate_protocol("chat").is_ok());
        assert!(SubprotocolList::validate_protocol("chat-v2").is_ok());
        assert!(SubprotocolList::validate_protocol("chat_v2").is_ok());
        assert!(SubprotocolList::validate_protocol("MQTT").is_ok());
        assert!(SubprotocolList::validate_protocol("graphql-ws").is_ok());
    }

    #[test]
    fn test_subprotocol_list_validate_empty() {
        let result = SubprotocolList::validate_protocol("");
        assert!(result.is_err());
    }

    #[test]
    fn test_subprotocol_list_validate_invalid_chars() {
        assert!(SubprotocolList::validate_protocol("chat.v2").is_err());
        assert!(SubprotocolList::validate_protocol("chat/v2").is_err());
        assert!(SubprotocolList::validate_protocol("chat v2").is_err());
        assert!(SubprotocolList::validate_protocol("chat@v2").is_err());
    }

    #[test]
    fn test_subprotocol_list_display() {
        let list = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap();

        assert_eq!(format!("{}", list), "chat-v2, mqtt");
    }

    #[test]
    fn test_subprotocol_list_case_sensitive() {
        let list = SubprotocolList::new()
            .with_protocol("MQTT")
            .unwrap();

        assert!(list.contains("MQTT"));
        assert!(!list.contains("mqtt"));
        assert!(!list.contains("Mqtt"));
    }

    // ===== SubprotocolNegotiator Tests =====

    #[test]
    fn test_negotiator_new() {
        let negotiator = SubprotocolNegotiator::new();
        assert!(negotiator.is_empty());
        assert_eq!(negotiator.len(), 0);
    }

    #[test]
    fn test_negotiator_from_slice() {
        let negotiator = SubprotocolNegotiator::from_slice(&["mqtt", "chat-v2"]).unwrap();

        assert_eq!(negotiator.len(), 2);
        assert_eq!(
            negotiator.protocols(),
            &["mqtt".to_string(), "chat-v2".to_string()]
        );
    }

    #[test]
    fn test_negotiator_with_protocol() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap();

        assert_eq!(negotiator.len(), 2);
    }

    #[test]
    fn test_negotiator_supports() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap();

        assert!(negotiator.supports("mqtt"));
        assert!(!negotiator.supports("graphql"));
    }

    #[test]
    fn test_negotiate_first_match() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("chat-v1")
            .unwrap();

        let client_protocols = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("graphql")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap();

        // Should select "chat-v2" (first match in client preference order)
        assert_eq!(
            negotiator.negotiate(&client_protocols),
            Some("chat-v2".to_string())
        );
    }

    #[test]
    fn test_negotiate_second_match() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap()
            .with_protocol("chat-v1")
            .unwrap();

        let client_protocols = SubprotocolList::new()
            .with_protocol("graphql")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap()
            .with_protocol("chat-v1")
            .unwrap();

        // Should select "mqtt" (first match)
        assert_eq!(
            negotiator.negotiate(&client_protocols),
            Some("mqtt".to_string())
        );
    }

    #[test]
    fn test_negotiate_no_match() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap();

        let client_protocols = SubprotocolList::new()
            .with_protocol("graphql")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap();

        // No match found
        assert_eq!(negotiator.negotiate(&client_protocols), None);
    }

    #[test]
    fn test_negotiate_client_empty() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap();

        let client_protocols = SubprotocolList::new();

        // Client offers no protocols
        assert_eq!(negotiator.negotiate(&client_protocols), None);
    }

    #[test]
    fn test_negotiate_server_empty() {
        let negotiator = SubprotocolNegotiator::new();

        let client_protocols = SubprotocolList::new()
            .with_protocol("mqtt")
            .unwrap();

        // Server supports no protocols
        assert_eq!(negotiator.negotiate(&client_protocols), None);
    }

    #[test]
    fn test_negotiate_wildcard() {
        let negotiator = SubprotocolNegotiator::new()
            .with_wildcard(true);

        let client_protocols = SubprotocolList::new()
            .with_protocol("any-protocol")
            .unwrap()
            .with_protocol("another-protocol")
            .unwrap();

        // With wildcard, accepts client's first preference
        assert_eq!(
            negotiator.negotiate(&client_protocols),
            Some("any-protocol".to_string())
        );
    }

    #[test]
    fn test_negotiate_wildcard_disabled() {
        let negotiator = SubprotocolNegotiator::new()
            .with_wildcard(false);

        let client_protocols = SubprotocolList::new()
            .with_protocol("any-protocol")
            .unwrap();

        // Without wildcard and no supported protocols, returns None
        assert_eq!(negotiator.negotiate(&client_protocols), None);
    }

    #[test]
    fn test_should_reject_true() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap();

        let client_protocols = SubprotocolList::new()
            .with_protocol("graphql")
            .unwrap();

        // Client offered protocols but no match
        assert!(negotiator.should_reject(&client_protocols));
    }

    #[test]
    fn test_should_reject_false_match() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap();

        let client_protocols = SubprotocolList::new()
            .with_protocol("mqtt")
            .unwrap();

        // Match found
        assert!(!negotiator.should_reject(&client_protocols));
    }

    #[test]
    fn test_should_reject_false_empty() {
        let negotiator = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap();

        let client_protocols = SubprotocolList::new();

        // Client offered no protocols
        assert!(!negotiator.should_reject(&client_protocols));
    }

    #[test]
    fn test_parse_header() {
        let header = "chat-v2, mqtt, graphql";
        let list = SubprotocolNegotiator::parse_header(header);

        assert_eq!(list.len(), 3);
        assert_eq!(
            list.protocols(),
            &["chat-v2".to_string(), "mqtt".to_string(), "graphql".to_string()]
        );
    }

    #[test]
    fn test_parse_header_with_spaces() {
        let header = "chat-v2 ,  mqtt  ,   graphql";
        let list = SubprotocolNegotiator::parse_header(header);

        assert_eq!(list.len(), 3);
        assert_eq!(
            list.protocols(),
            &["chat-v2".to_string(), "mqtt".to_string(), "graphql".to_string()]
        );
    }

    #[test]
    fn test_parse_header_empty() {
        let list = SubprotocolNegotiator::parse_header("");
        assert!(list.is_empty());
    }

    #[test]
    fn test_parse_header_invalid_protocols() {
        let header = "valid-v1, invalid.v2, valid-v3";
        let list = SubprotocolNegotiator::parse_header(header);

        // Should skip invalid protocols
        assert_eq!(list.len(), 2);
        assert_eq!(
            list.protocols(),
            &["valid-v1".to_string(), "valid-v3".to_string()]
        );
    }

    // ===== Integration Tests =====

    #[test]
    fn test_full_negotiation_flow() {
        // Server configuration
        let server = SubprotocolNegotiator::new()
            .with_protocol("mqtt")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("chat-v1")
            .unwrap();

        // Client request
        let client = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("graphql")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap();

        // Server selects protocol
        let selected = server.negotiate(&client);
        assert_eq!(selected, Some("chat-v2".to_string()));

        // Generate header
        let header_value = client.to_header_value();
        assert_eq!(header_value, "chat-v2, graphql, mqtt");

        // Client validates server response
        let validated = client.parse_response(selected.as_deref()).unwrap();
        assert_eq!(validated, Some("chat-v2".to_string()));
    }

    #[test]
    fn test_no_subprotocol_negotiation() {
        // Server doesn't require subprotocol
        let server = SubprotocolNegotiator::new();

        // Client doesn't offer subprotocol
        let client = SubprotocolList::new();

        // No negotiation
        let selected = server.negotiate(&client);
        assert_eq!(selected, None);

        // Client accepts no protocol
        let validated = client.parse_response(None).unwrap();
        assert_eq!(validated, None);
    }

    #[test]
    fn test_priority_order_matters() {
        let server = SubprotocolNegotiator::new()
            .with_protocol("chat-v1")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("chat-v3")
            .unwrap();

        // Client prefers v3, then v2, then v1
        let client1 = SubprotocolList::new()
            .with_protocol("chat-v3")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("chat-v1")
            .unwrap();

        // Should get v3 (first match)
        assert_eq!(
            server.negotiate(&client1),
            Some("chat-v3".to_string())
        );

        // Client prefers v1, then v2, then v3
        let client2 = SubprotocolList::new()
            .with_protocol("chat-v1")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("chat-v3")
            .unwrap();

        // Should get v1 (first match)
        assert_eq!(
            server.negotiate(&client2),
            Some("chat-v1".to_string())
        );
    }

    #[test]
    fn test_duplicate_protocols() {
        // Client sends duplicate protocols
        let client = SubprotocolList::new()
            .with_protocol("chat-v2")
            .unwrap()
            .with_protocol("mqtt")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap(); // Duplicate

        assert_eq!(client.len(), 3); // Doesn't deduplicate
        assert_eq!(client.to_header_value(), "chat-v2, mqtt, chat-v2");
    }

    #[test]
    fn test_common_protocols() {
        let mqtt = SubprotocolList::new()
            .with_protocol("mqtt")
            .unwrap();

        let graphql = SubprotocolList::new()
            .with_protocol("graphql")
            .unwrap()
            .with_protocol("graphql-transport-ws")
            .unwrap();

        let chat = SubprotocolList::new()
            .with_protocol("chat-v1")
            .unwrap()
            .with_protocol("chat-v2")
            .unwrap();

        // Common real-world subprotocols
        assert!(mqtt.contains("mqtt"));
        assert!(graphql.contains("graphql"));
        assert!(chat.contains("chat-v2"));
    }
}

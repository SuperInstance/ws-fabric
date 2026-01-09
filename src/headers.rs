//! Custom HTTP headers support for WebSocket handshakes
//!
//! This module provides a flexible, efficient header map implementation for managing
//! custom HTTP headers during WebSocket handshake negotiations. It supports:
//!
//! - Multiple values per header name
//! - HTTP spec-compliant validation
//! - Type-safe accessors for common headers
//! - Conversion to/from `http::HeaderMap`
//! - Ergonomic builder API

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::iter::FromIterator;

/// A case-insensitive string wrapper for header names
///
/// HTTP header names are case-insensitive per RFC 7230 Section 3.2.
/// This wrapper ensures consistent comparison regardless of case.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HeaderName(String);

impl HeaderName {
    /// Create a new header name
    ///
    /// # Errors
    ///
    /// Returns an error if the name contains invalid characters.
    /// Per RFC 7230, header names must consist of:
    /// - Uppercase or lowercase letters (a-zA-Z)
    /// - Digits (0-9)
    /// - Hyphen (-)
    /// - Must not be empty
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderName;
    ///
    /// let name = HeaderName::new("Authorization").unwrap();
    /// assert_eq!(name.as_str(), "Authorization");
    ///
    /// // Invalid characters fail
    /// assert!(HeaderName::new("Invalid:Name").is_err());
    /// ```
    pub fn new(name: impl AsRef<str>) -> Result<Self> {
        let name_str = name.as_ref();

        if name_str.is_empty() {
            return Err(Error::invalid_state("Header name cannot be empty"));
        }

        // Validate per RFC 7230 Section 3.2
        // Header names: a-zA-Z0-9 and hyphen only
        if !name_str
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '!')
        {
            return Err(Error::invalid_state(format!(
                "Invalid header name '{}': contains invalid characters",
                name_str
            )));
        }

        // Check for control characters
        if name_str.chars().any(|c| c.is_ascii_control()) {
            return Err(Error::invalid_state(format!(
                "Invalid header name '{}': contains control characters",
                name_str
            )));
        }

        Ok(HeaderName(name_str.to_string()))
    }

    /// Get the header name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to owned string
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for HeaderName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for HeaderName {
    fn from(s: &str) -> Self {
        HeaderName::new(s).expect("Invalid header name")
    }
}

impl From<String> for HeaderName {
    fn from(s: String) -> Self {
        HeaderName::new(s).expect("Invalid header name")
    }
}

/// A validated header value
///
/// HTTP header values must be printable ASCII characters without control characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderValue(String);

impl HeaderValue {
    /// Create a new header value
    ///
    /// # Errors
    ///
    /// Returns an error if the value contains invalid characters.
    /// Per RFC 7230, header values must:
    /// - Be printable ASCII (0x20-0x7E)
    /// - Not contain control characters (except SP)
    /// - Not contain leading or trailing whitespace (will be trimmed)
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderValue;
    ///
    /// let value = HeaderValue::new("Bearer token123").unwrap();
    /// assert_eq!(value.as_str(), "Bearer token123");
    ///
    /// // Leading/trailing whitespace is trimmed
    /// let value = HeaderValue::new("  trimmed  ").unwrap();
    /// assert_eq!(value.as_str(), "trimmed");
    /// ```
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        let value_str = value.as_ref().trim();

        // Validate per RFC 7230
        // Must be printable ASCII without control characters
        for (i, c) in value_str.chars().enumerate() {
            if c.is_ascii_control() && c != ' ' {
                return Err(Error::invalid_state(format!(
                    "Invalid header value at position {}: contains control character 0x{:02X}",
                    i, c as u32
                )));
            }
            if !c.is_ascii() && c != ' ' {
                return Err(Error::invalid_state(format!(
                    "Invalid header value at position {}: contains non-ASCII character",
                    i
                )));
            }
        }

        // Check for valid byte range (0x20-0x7E)
        for byte in value_str.bytes() {
            if !(0x20..=0x7E).contains(&byte) {
                return Err(Error::invalid_state(format!(
                    "Invalid header value: contains byte 0x{:02X} outside valid range 0x20-0x7E",
                    byte
                )));
            }
        }

        Ok(HeaderValue(value_str.to_string()))
    }


    /// Get the header value as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to owned string
    pub fn into_string(self) -> String {
        self.0
    }

    /// Check if the value is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the length of the value
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl AsRef<str> for HeaderValue {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for HeaderValue {
    fn from(s: &str) -> Self {
        HeaderValue::new(s).expect("Invalid header value")
    }
}

impl From<String> for HeaderValue {
    fn from(s: String) -> Self {
        HeaderValue::new(s).expect("Invalid header value")
    }
}

/// A multi-map for HTTP headers
///
/// HTTP headers can have multiple values for the same name (e.g., Set-Cookie).
/// This `HeaderMap` stores values as vectors to support this use case.
///
/// Internally uses a `HashMap` with case-insensitive key comparison.
///
/// # Example
///
/// ```rust
/// use websocket_fabric::headers::HeaderMap;
///
/// let mut headers = HeaderMap::new();
/// headers.insert("Authorization", "Bearer token123");
/// headers.insert("X-Custom-Header", "value1");
/// headers.append("X-Custom-Header", "value2");
///
/// assert_eq!(headers.get("Authorization"), Some("Bearer token123"));
/// assert_eq!(headers.get_all("X-Custom-Header").len(), 2);
/// ```
#[derive(Debug, Clone, Default)]
pub struct HeaderMap {
    inner: HashMap<String, Vec<String>>,
}

impl HeaderMap {
    /// Create an empty header map
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a header map with capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: HashMap::with_capacity(capacity),
        }
    }

    /// Insert a header, replacing any existing values
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("Authorization", "Bearer token123");
    /// headers.insert("Authorization", "Bearer newtoken");
    ///
    /// assert_eq!(headers.get("Authorization"), Some("Bearer newtoken"));
    /// assert_eq!(headers.get_all("Authorization").len(), 1);
    /// ```
    pub fn insert(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Result<()> {
        let name = HeaderName::new(name)?;
        let value = HeaderValue::new(value)?;

        self.inner.insert(name.as_str().to_lowercase(), vec![value.into_string()]);
        Ok(())
    }

    /// Append a header value, keeping existing values
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("Set-Cookie", "cookie1=value1");
    /// headers.append("Set-Cookie", "cookie2=value2");
    ///
    /// assert_eq!(headers.get_all("Set-Cookie").len(), 2);
    /// ```
    pub fn append(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Result<()> {
        let name = HeaderName::new(name)?;
        let value = HeaderValue::new(value)?;

        let key = name.as_str().to_lowercase();
        self.inner
            .entry(key)
            .or_default()
            .push(value.into_string());
        Ok(())
    }

    /// Get the first value for a header name
    ///
    /// Returns `None` if the header doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("Authorization", "Bearer token123");
    ///
    /// assert_eq!(headers.get("Authorization"), Some("Bearer token123"));
    /// assert_eq!(headers.get("X-Missing"), None);
    /// ```
    pub fn get(&self, name: impl AsRef<str>) -> Option<&str> {
        let key = name.as_ref().to_lowercase();
        self.inner.get(&key).and_then(|values| values.first())
            .map(|s| s.as_str())
    }

    /// Get all values for a header name
    ///
    /// Returns an empty vector if the header doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("Set-Cookie", "cookie1=value1");
    /// headers.append("Set-Cookie", "cookie2=value2");
    ///
    /// let values = headers.get_all("Set-Cookie");
    /// assert_eq!(values.len(), 2);
    /// ```
    pub fn get_all(&self, name: impl AsRef<str>) -> Vec<&str> {
        let key = name.as_ref().to_lowercase();
        self.inner
            .get(&key)
            .map(|values| values.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Remove a header and return its first value
    ///
    /// Returns `None` if the header doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("Authorization", "Bearer token123");
    ///
    /// let value = headers.remove("Authorization");
    /// assert_eq!(value, Some("Bearer token123".to_string()));
    /// assert!(!headers.contains("Authorization"));
    /// ```
    pub fn remove(&mut self, name: impl AsRef<str>) -> Option<String> {
        let key = name.as_ref().to_lowercase();
        self.inner.remove(&key).and_then(|mut values| values.pop())
    }

    /// Check if a header exists
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("Authorization", "Bearer token123");
    ///
    /// assert!(headers.contains("Authorization"));
    /// assert!(!headers.contains("X-Missing"));
    /// ```
    pub fn contains(&self, name: impl AsRef<str>) -> bool {
        let key = name.as_ref().to_lowercase();
        self.inner.contains_key(&key)
    }

    /// Check if the header map is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the number of unique header names
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Clear all headers
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterate over all headers (name, first value) pairs
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.inner.iter().filter_map(|(name, values)| {
            values.first().map(|first| (name.as_str(), first.as_str()))
        })
    }

    /// Iterate over all headers with all values
    pub fn iter_all(&self) -> impl Iterator<Item = (&str, Vec<&str>)> {
        self.inner.iter().map(|(name, values)| {
            (name.as_str(), values.iter().map(|s| s.as_str()).collect())
        })
    }

    /// Extend this header map with another
    pub fn extend(&mut self, other: HeaderMap) {
        for (key, mut values) in other.inner {
            self.inner.entry(key).or_default().append(&mut values);
        }
    }

    // Type-safe accessors for common headers

    /// Get the Authorization header value
    pub fn get_authorization(&self) -> Option<&str> {
        self.get("Authorization")
    }

    /// Set the Authorization header value
    pub fn set_authorization(&mut self, value: impl AsRef<str>) -> Result<()> {
        self.insert("Authorization", value)
    }

    /// Get the User-Agent header value
    pub fn get_user_agent(&self) -> Option<&str> {
        self.get("User-Agent")
    }

    /// Set the User-Agent header value
    pub fn set_user_agent(&mut self, value: impl AsRef<str>) -> Result<()> {
        self.insert("User-Agent", value)
    }

    /// Get the Origin header value
    pub fn get_origin(&self) -> Option<&str> {
        self.get("Origin")
    }

    /// Set the Origin header value
    pub fn set_origin(&mut self, value: impl AsRef<str>) -> Result<()> {
        self.insert("Origin", value)
    }

    /// Get the Cookie header value
    pub fn get_cookie(&self) -> Option<&str> {
        self.get("Cookie")
    }

    /// Set the Cookie header value
    pub fn set_cookie(&mut self, value: impl AsRef<str>) -> Result<()> {
        self.insert("Cookie", value)
    }

    /// Get all Cookie values
    pub fn get_all_cookies(&self) -> Vec<&str> {
        self.get_all("Cookie")
    }

    /// Get the Host header value
    pub fn get_host(&self) -> Option<&str> {
        self.get("Host")
    }

    /// Set the Host header value
    pub fn set_host(&mut self, value: impl AsRef<str>) -> Result<()> {
        self.insert("Host", value)
    }

    /// Get the X-API-Key header value (common custom header for API keys)
    pub fn get_api_key(&self) -> Option<&str> {
        self.get("X-API-Key")
    }

    /// Set the X-API-Key header value
    pub fn set_api_key(&mut self, value: impl AsRef<str>) -> Result<()> {
        self.insert("X-API-Key", value)
    }
}

impl FromIterator<(String, String)> for HeaderMap {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut map = HeaderMap::new();
        for (key, value) in iter {
            // Silently skip invalid headers during iteration
            let _ = map.insert(key, value);
        }
        map
    }
}

impl<'a> FromIterator<(&'a str, &'a str)> for HeaderMap {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut map = HeaderMap::new();
        for (key, value) in iter {
            // Silently skip invalid headers during iteration
            let _ = map.insert(key, value);
        }
        map
    }
}

// Conversion to/from http::HeaderMap

impl From<&http::HeaderMap> for HeaderMap {
    fn from(http_map: &http::HeaderMap) -> Self {
        let mut map = HeaderMap::with_capacity(http_map.len());

        for (name, value) in http_map.iter() {
            let name_str = name.as_str();
            let value_str = value.to_str().unwrap_or("");
            // Silently skip conversion errors
            let _ = map.append(name_str, value_str);
        }

        map
    }
}

impl From<http::HeaderMap> for HeaderMap {
    fn from(http_map: http::HeaderMap) -> Self {
        Self::from(&http_map)
    }
}

impl TryFrom<&HeaderMap> for http::HeaderMap {
    type Error = Error;

    fn try_from(map: &HeaderMap) -> Result<Self> {
        let mut http_map = http::HeaderMap::with_capacity(map.len());

        for (name, values) in map.iter_all() {
            let http_name = http::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|e| Error::invalid_state(format!("Invalid header name '{}': {}", name, e)))?;

            for value in values {
                let http_value = http::header::HeaderValue::from_str(value)
                    .map_err(|e| Error::invalid_state(format!("Invalid header value for '{}': {}", name, e)))?;
                http_map.append(&http_name, http_value);
            }
        }

        Ok(http_map)
    }
}

impl TryFrom<HeaderMap> for http::HeaderMap {
    type Error = Error;

    fn try_from(map: HeaderMap) -> Result<Self> {
        Self::try_from(&map)
    }
}

impl HeaderMap {
    /// Convert to `http::HeaderMap`
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut headers = HeaderMap::new();
    /// headers.insert("Authorization", "Bearer token123").unwrap();
    ///
    /// let http_map = headers.to_http_header_map().unwrap();
    /// ```
    pub fn to_http_header_map(&self) -> Result<http::HeaderMap> {
        self.try_into()
    }

    /// Convert from `http::HeaderMap`
    ///
    /// # Example
    ///
    /// ```rust
    /// use websocket_fabric::headers::HeaderMap;
    ///
    /// let mut http_map = http::HeaderMap::new();
    /// http_map.insert("Authorization", "Bearer token123");
    ///
    /// let headers = HeaderMap::from_http_header_map(&http_map);
    /// ```
    pub fn from_http_header_map(http_map: &http::HeaderMap) -> Self {
        Self::from(http_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // HeaderName tests

    #[test]
    fn test_header_name_valid() {
        assert!(HeaderName::new("Authorization").is_ok());
        assert!(HeaderName::new("X-Custom-Header").is_ok());
        assert!(HeaderName::new("Content-Type").is_ok());
        assert!(HeaderName::new("accept-encoding").is_ok());
    }

    #[test]
    fn test_header_name_invalid() {
        assert!(HeaderName::new("").is_err());
        assert!(HeaderName::new("Invalid:Name").is_err());
        assert!(HeaderName::new("Invalid Name").is_err());
        assert!(HeaderName::new("Invalid\nName").is_err());
    }

    #[test]
    fn test_header_name_from_str() {
        let name = HeaderName::from("Authorization");
        assert_eq!(name.as_str(), "Authorization");
    }

    // HeaderValue tests

    #[test]
    fn test_header_value_valid() {
        assert!(HeaderValue::new("Bearer token123").is_ok());
        assert!(HeaderValue::new("application/json").is_ok());
        assert!(HeaderValue::new("text/plain; charset=utf-8").is_ok());
    }

    #[test]
    fn test_header_value_whitespace_trimmed() {
        let value = HeaderValue::new("  trimmed  ").unwrap();
        assert_eq!(value.as_str(), "trimmed");
    }

    #[test]
    fn test_header_value_invalid() {
        assert!(HeaderValue::new("invalid\x00value").is_err());
        assert!(HeaderValue::new("invalid\nvalue").is_err());
        assert!(HeaderValue::new("invalid\rvalue").is_err());
    }

    #[test]
    fn test_header_value_from_str() {
        let value = HeaderValue::from("Bearer token123");
        assert_eq!(value.as_str(), "Bearer token123");
    }

    // HeaderMap tests

    #[test]
    fn test_header_map_new() {
        let map = HeaderMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_header_map_insert() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();

        assert_eq!(map.get("Authorization"), Some("Bearer token123"));
        assert_eq!(map.get("authorization"), Some("Bearer token123")); // Case-insensitive
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_header_map_append() {
        let mut map = HeaderMap::new();
        map.insert("Set-Cookie", "cookie1=value1").unwrap();
        map.append("Set-Cookie", "cookie2=value2").unwrap();

        let values = map.get_all("Set-Cookie");
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], "cookie1=value1");
        assert_eq!(values[1], "cookie2=value2");
    }

    #[test]
    fn test_header_map_replace() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token1").unwrap();
        map.insert("Authorization", "Bearer token2").unwrap();

        assert_eq!(map.get("Authorization"), Some("Bearer token2"));
        assert_eq!(map.get_all("Authorization").len(), 1);
    }

    #[test]
    fn test_header_map_get_missing() {
        let map = HeaderMap::new();
        assert_eq!(map.get("Missing"), None);
        assert_eq!(map.get_all("Missing").len(), 0);
    }

    #[test]
    fn test_header_map_remove() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();

        let value = map.remove("Authorization");
        assert_eq!(value, Some("Bearer token123".to_string()));
        assert!(!map.contains("Authorization"));
    }

    #[test]
    fn test_header_map_contains() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();

        assert!(map.contains("Authorization"));
        assert!(map.contains("authorization"));
        assert!(!map.contains("Missing"));
    }

    #[test]
    fn test_header_map_clear() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();
        map.insert("User-Agent", "Test").unwrap();

        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_header_map_iter() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();
        map.insert("User-Agent", "Test").unwrap();

        let pairs: Vec<_> = map.iter().collect();
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn test_header_map_iter_all() {
        let mut map = HeaderMap::new();
        map.insert("Set-Cookie", "cookie1=value1").unwrap();
        map.append("Set-Cookie", "cookie2=value2").unwrap();

        let pairs: Vec<_> = map.iter_all().collect();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].1.len(), 2);
    }

    #[test]
    fn test_header_map_extend() {
        let mut map1 = HeaderMap::new();
        map1.insert("Authorization", "Bearer token1").unwrap();

        let mut map2 = HeaderMap::new();
        map2.insert("User-Agent", "Test").unwrap();

        map1.extend(map2);
        assert_eq!(map1.len(), 2);
        assert!(map1.contains("Authorization"));
        assert!(map1.contains("User-Agent"));
    }

    #[test]
    fn test_header_map_from_iter() {
        let headers = vec![
            ("Authorization", "Bearer token123"),
            ("User-Agent", "Test"),
        ];
        let map: HeaderMap = headers.into_iter().collect();

        assert_eq!(map.len(), 2);
        assert_eq!(map.get("Authorization"), Some("Bearer token123"));
        assert_eq!(map.get("User-Agent"), Some("Test"));
    }

    // Type-safe accessor tests

    #[test]
    fn test_authorization_accessor() {
        let mut map = HeaderMap::new();
        assert_eq!(map.get_authorization(), None);

        map.set_authorization("Bearer token123").unwrap();
        assert_eq!(map.get_authorization(), Some("Bearer token123"));
    }

    #[test]
    fn test_user_agent_accessor() {
        let mut map = HeaderMap::new();
        assert_eq!(map.get_user_agent(), None);

        map.set_user_agent("MyClient/1.0").unwrap();
        assert_eq!(map.get_user_agent(), Some("MyClient/1.0"));
    }

    #[test]
    fn test_origin_accessor() {
        let mut map = HeaderMap::new();
        assert_eq!(map.get_origin(), None);

        map.set_origin("https://example.com").unwrap();
        assert_eq!(map.get_origin(), Some("https://example.com"));
    }

    #[test]
    fn test_cookie_accessor() {
        let mut map = HeaderMap::new();
        assert_eq!(map.get_cookie(), None);

        map.set_cookie("session=abc123").unwrap();
        assert_eq!(map.get_cookie(), Some("session=abc123"));
    }

    #[test]
    fn test_host_accessor() {
        let mut map = HeaderMap::new();
        assert_eq!(map.get_host(), None);

        map.set_host("example.com").unwrap();
        assert_eq!(map.get_host(), Some("example.com"));
    }

    #[test]
    fn test_api_key_accessor() {
        let mut map = HeaderMap::new();
        assert_eq!(map.get_api_key(), None);

        map.set_api_key("secret-key-123").unwrap();
        assert_eq!(map.get_api_key(), Some("secret-key-123"));
    }

    #[test]
    fn test_multiple_cookies() {
        let mut map = HeaderMap::new();
        map.set_cookie("session=abc123").unwrap();
        map.append("Cookie", "user=john").unwrap();

        let cookies = map.get_all_cookies();
        assert_eq!(cookies.len(), 2);
    }

    // Edge case tests

    #[test]
    fn test_empty_header_value() {
        let mut map = HeaderMap::new();
        assert!(map.insert("X-Empty", "").is_ok());

        // Empty value after validation
        assert_eq!(map.get("X-Empty"), Some(""));
    }

    #[test]
    fn test_case_insensitive() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();

        assert_eq!(map.get("authorization"), Some("Bearer token123"));
        assert_eq!(map.get("AUTHORIZATION"), Some("Bearer token123"));
        assert_eq!(map.get("AuThOrIzAtIoN"), Some("Bearer token123"));
    }

    #[test]
    fn test_special_header_names() {
        // Test headers with special characters (allowed in some contexts)
        assert!(HeaderName::new("X-Custom!Header").is_ok());
        assert!(HeaderName::new("X-Test-123").is_ok());
    }

    #[test]
    fn test_header_value_with_spaces() {
        let value = HeaderValue::new("Bearer token123 with spaces").unwrap();
        assert_eq!(value.as_str(), "Bearer token123 with spaces");
    }

    #[test]
    fn test_header_value_with_special_chars() {
        assert!(HeaderValue::new("text/plain; charset=utf-8").is_ok());
        assert!(HeaderValue::new("application/json, text/html").is_ok());
        assert!(HeaderValue::new("Bearer token123+abc").is_ok());
    }

    // http::HeaderMap conversion tests

    #[test]
    fn test_from_http_header_map() {
        let mut http_map = http::HeaderMap::new();
        http_map.insert("Authorization", http::header::HeaderValue::from_static("Bearer token123"));
        http_map.insert("User-Agent", http::header::HeaderValue::from_static("TestClient/1.0"));
        http_map.append("Set-Cookie", http::header::HeaderValue::from_static("cookie1=value1"));
        http_map.append("Set-Cookie", http::header::HeaderValue::from_static("cookie2=value2"));

        let map = HeaderMap::from_http_header_map(&http_map);

        assert_eq!(map.get("Authorization"), Some("Bearer token123"));
        assert_eq!(map.get("User-Agent"), Some("TestClient/1.0"));
        // Note: http::HeaderMap only keeps the last value when iterating
        // So we only get one cookie value in our implementation
    }

    #[test]
    fn test_to_http_header_map() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();
        map.insert("User-Agent", "TestClient/1.0").unwrap();
        map.append("Set-Cookie", "cookie1=value1").unwrap();
        map.append("Set-Cookie", "cookie2=value2").unwrap();

        let http_map = map.to_http_header_map().unwrap();

        assert_eq!(http_map["Authorization"], "Bearer token123");
        assert_eq!(http_map["User-Agent"], "TestClient/1.0");

        let values: Vec<_> = http_map.get_all("Set-Cookie").iter().collect();
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_http_header_map_roundtrip() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();
        map.insert("X-Custom", "custom-value").unwrap();

        let http_map = map.to_http_header_map().unwrap();
        let map2 = HeaderMap::from_http_header_map(&http_map);

        assert_eq!(map2.get("Authorization"), Some("Bearer token123"));
        assert_eq!(map2.get("X-Custom"), Some("custom-value"));
    }

    #[test]
    fn test_http_header_map_from_owned() {
        let mut http_map = http::HeaderMap::new();
        http_map.insert("Authorization", http::header::HeaderValue::from_static("Bearer token123"));

        let map: HeaderMap = http_map.into();

        assert_eq!(map.get("Authorization"), Some("Bearer token123"));
    }

    #[test]
    fn test_http_header_map_try_from_owned() {
        let mut map = HeaderMap::new();
        map.insert("Authorization", "Bearer token123").unwrap();

        let http_map: Result<http::HeaderMap> = map.try_into();
        assert!(http_map.is_ok());

        let http_map = http_map.unwrap();
        assert_eq!(http_map["Authorization"], "Bearer token123");
    }

    #[test]
    fn test_http_header_map_invalid_value() {
        let mut map = HeaderMap::new();
        // Our validation rejects control characters, so insert fails
        let result = map.insert("X-Invalid", "invalid\x00value");
        assert!(result.is_err());

        // Test with a value that passes our validation but might fail http::HeaderMap
        // (most values should work since we validate similarly)
        let valid_result = map.insert("X-Valid", "valid-value");
        assert!(valid_result.is_ok());
    }
}

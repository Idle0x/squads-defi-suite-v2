use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A test transport that returns pre-loaded responses for specific URLs.
/// Implements the same interface waki would use, but returns fixture data.
///
/// This allows testing Jupiter quote parsing, guardrail validation,
/// and proposal builder WITHOUT network calls.
#[derive(Clone)]
pub struct FileBackedMockTransport {
    /// Map of URL pattern → response JSON string
    responses: Arc<Mutex<HashMap<String, String>>>,
    /// Map of URL pattern → error to return (for error-case testing)
    errors: Arc<Mutex<HashMap<String, String>>>,
}

impl FileBackedMockTransport {
    pub fn new() -> Self {
        FileBackedMockTransport {
            responses: Arc::new(Mutex::new(HashMap::new())),
            errors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a URL pattern → JSON response string
    pub fn register_response(&self, url_pattern: &str, json_response: &str) {
        self.responses
            .lock()
            .unwrap()
            .insert(url_pattern.to_string(), json_response.to_string());
    }

    /// Register a URL pattern → error message
    pub fn register_error(&self, url_pattern: &str, error_message: &str) {
        self.errors
            .lock()
            .unwrap()
            .insert(url_pattern.to_string(), error_message.to_string());
    }

    /// Simulate a GET request — returns the registered JSON response
    /// for a matching URL pattern, or an error if one is registered.
    pub fn get(&self, url: &str) -> Result<Value, String> {
        // Check for registered errors first
        for (pattern, error_msg) in self.errors.lock().unwrap().iter() {
            if url.contains(pattern) {
                return Err(error_msg.clone());
            }
        }

        // Check for registered responses
        for (pattern, json_str) in self.responses.lock().unwrap().iter() {
            if url.contains(pattern) {
                return serde_json::from_str(json_str)
                    .map_err(|e| format!("Failed to parse fixture JSON: {}", e));
            }
        }

        Err(format!("No mock response registered for URL: {}", url))
    }
}

impl Default for FileBackedMockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_registers_and_returns_response() {
        let mock = FileBackedMockTransport::new();
        mock.register_response("api.jup.ag", r#"{"status":"ok"}"#);
        let result = mock.get("https://api.jup.ag/quote").unwrap();
        assert_eq!(result["status"], "ok");
    }

    #[test]
    fn test_mock_registers_error() {
        let mock = FileBackedMockTransport::new();
        mock.register_error("timeout", "Request timed out");
        let result = mock.get("https://timeout.example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timed out"));
    }

    #[test]
    fn test_mock_no_match() {
        let mock = FileBackedMockTransport::new();
        let result = mock.get("https://unknown.example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No mock response"));
    }
}

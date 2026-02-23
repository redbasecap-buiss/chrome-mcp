use thiserror::Error;

/// Main error type for chrome-mcp
#[derive(Error, Debug)]
pub enum ChromeMcpError {
    #[error("CDP connection error: {0}")]
    CdpConnection(String),

    #[error("CDP protocol error: {0}")]
    CdpProtocol(String),

    #[error("Element not found: {0}")]
    ElementNotFound(String),

    #[error("Navigation timeout: {0}")]
    NavigationTimeout(String),

    #[error("JavaScript evaluation error: {0}")]
    JavaScriptError(String),

    #[error("Screenshot capture error: {0}")]
    Screenshot(String),

    #[error("Network interception error: {0}")]
    Network(String),

    #[error("Accessibility tree error: {0}")]
    Accessibility(String),

    #[error("Native input error: {0}")]
    NativeInput(String),

    #[error("MCP protocol error: {0}")]
    McpProtocol(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Tab not found: {0}")]
    TabNotFound(String),

    #[error("Timeout: operation timed out after {timeout}ms")]
    Timeout { timeout: u64 },
}

pub type Result<T> = std::result::Result<T, ChromeMcpError>;

impl ChromeMcpError {
    pub fn cdp_connection(msg: impl Into<String>) -> Self {
        Self::CdpConnection(msg.into())
    }

    pub fn cdp_protocol(msg: impl Into<String>) -> Self {
        Self::CdpProtocol(msg.into())
    }

    pub fn element_not_found(msg: impl Into<String>) -> Self {
        Self::ElementNotFound(msg.into())
    }

    pub fn navigation_timeout(msg: impl Into<String>) -> Self {
        Self::NavigationTimeout(msg.into())
    }

    pub fn javascript_error(msg: impl Into<String>) -> Self {
        Self::JavaScriptError(msg.into())
    }

    pub fn screenshot_error(msg: impl Into<String>) -> Self {
        Self::Screenshot(msg.into())
    }

    pub fn network_error(msg: impl Into<String>) -> Self {
        Self::Network(msg.into())
    }

    pub fn accessibility_error(msg: impl Into<String>) -> Self {
        Self::Accessibility(msg.into())
    }

    pub fn native_input_error(msg: impl Into<String>) -> Self {
        Self::NativeInput(msg.into())
    }

    pub fn mcp_protocol_error(msg: impl Into<String>) -> Self {
        Self::McpProtocol(msg.into())
    }

    pub fn invalid_operation(msg: impl Into<String>) -> Self {
        Self::InvalidOperation(msg.into())
    }

    pub fn tab_not_found(msg: impl Into<String>) -> Self {
        Self::TabNotFound(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_creation_methods() {
        let error = ChromeMcpError::cdp_connection("connection failed");
        assert!(matches!(error, ChromeMcpError::CdpConnection(_)));
        assert_eq!(format!("{}", error), "CDP connection error: connection failed");

        let error = ChromeMcpError::cdp_protocol("invalid response");
        assert!(matches!(error, ChromeMcpError::CdpProtocol(_)));
        
        let error = ChromeMcpError::element_not_found("button#submit");
        assert!(matches!(error, ChromeMcpError::ElementNotFound(_)));
        
        let error = ChromeMcpError::navigation_timeout("page load");
        assert!(matches!(error, ChromeMcpError::NavigationTimeout(_)));
        
        let error = ChromeMcpError::javascript_error("syntax error");
        assert!(matches!(error, ChromeMcpError::JavaScriptError(_)));
    }

    #[test]
    fn test_all_error_variants() {
        let errors = vec![
            ChromeMcpError::screenshot_error("capture failed"),
            ChromeMcpError::network_error("request timeout"),
            ChromeMcpError::accessibility_error("tree parse error"),
            ChromeMcpError::native_input_error("permission denied"),
            ChromeMcpError::mcp_protocol_error("invalid message"),
            ChromeMcpError::invalid_operation("unsupported action"),
            ChromeMcpError::tab_not_found("tab123"),
        ];

        for error in errors {
            let error_string = format!("{}", error);
            assert!(!error_string.is_empty());
            assert!(error_string.len() > 5); // Ensure meaningful error messages
        }
    }

    #[test]
    fn test_timeout_error() {
        let error = ChromeMcpError::Timeout { timeout: 5000 };
        assert!(matches!(error, ChromeMcpError::Timeout { timeout: 5000 }));
        assert_eq!(format!("{}", error), "Timeout: operation timed out after 5000ms");
    }

    #[test]
    fn test_error_from_conversions() {
        // Test IO error conversion
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let chrome_error: ChromeMcpError = io_error.into();
        assert!(matches!(chrome_error, ChromeMcpError::Io(_)));

        // Test JSON error conversion
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let chrome_error: ChromeMcpError = json_error.into();
        assert!(matches!(chrome_error, ChromeMcpError::Json(_)));

        // Test URL error conversion
        let url_error = url::Url::parse("not_a_url").unwrap_err();
        let chrome_error: ChromeMcpError = url_error.into();
        assert!(matches!(chrome_error, ChromeMcpError::Url(_)));
    }

    #[test]
    fn test_result_type_usage() {
        fn success_function() -> Result<String> {
            Ok("success".to_string())
        }

        fn error_function() -> Result<String> {
            Err(ChromeMcpError::element_not_found("test"))
        }

        let result = success_function();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        let result = error_function();
        assert!(result.is_err());
        match result {
            Err(ChromeMcpError::ElementNotFound(msg)) => assert_eq!(msg, "test"),
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn test_error_debug_formatting() {
        let error = ChromeMcpError::cdp_connection("test");
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("CdpConnection"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_error_chain_compatibility() {
        // Test that errors work with the ? operator
        fn nested_function() -> Result<()> {
            let _io_error = std::fs::File::open("nonexistent_file.txt")?;
            Ok(())
        }

        let result = nested_function();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChromeMcpError::Io(_)));
    }
}
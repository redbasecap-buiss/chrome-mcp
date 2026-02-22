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
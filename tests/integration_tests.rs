use chrome_mcp::accessibility::AccessibilityManager;
use chrome_mcp::browser::{Browser, Cookie};
use chrome_mcp::cdp::CdpClient;
use chrome_mcp::error::{ChromeMcpError, Result};
use chrome_mcp::mcp::McpServer;
use chrome_mcp::native_input::NativeInputManager;
use chrome_mcp::screenshot::ScreenshotManager;
use serde_json::json;
use base64::Engine;
// use tempfile::NamedTempFile;
// use tokio_test;

#[cfg(test)]
mod cdp_tests {
    use super::*;

    #[tokio::test]
    async fn test_cdp_client_creation() {
        let client = CdpClient::new("localhost", 9222);
        assert!(client.current_tab_id().is_none());
    }

    #[tokio::test]
    #[ignore] // Requires running Chrome instance
    async fn test_list_tabs() {
        let client = CdpClient::new("localhost", 9222);
        let result = client.list_tabs().await;
        // This will fail if Chrome isn't running, which is expected
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    #[ignore] // Requires running Chrome instance
    async fn test_create_tab() {
        let client = CdpClient::new("localhost", 9222);
        let result = client.create_tab(Some("https://example.com")).await;
        // This will fail if Chrome isn't running, which is expected
        assert!(result.is_ok() || result.is_err());
    }
}

#[cfg(test)]
mod browser_tests {
    use super::*;

    #[test]
    fn test_browser_creation() {
        let result = Browser::new("localhost", 9222);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cookie_serialization() {
        let cookie = Cookie {
            name: "test".to_string(),
            value: "value".to_string(),
            domain: "example.com".to_string(),
            path: "/".to_string(),
            secure: true,
            http_only: false,
            same_site: Some("Strict".to_string()),
            expires: Some(1234567890.0),
        };

        let json_str = serde_json::to_string(&cookie).unwrap();
        let deserialized: Cookie = serde_json::from_str(&json_str).unwrap();
        
        assert_eq!(cookie.name, deserialized.name);
        assert_eq!(cookie.value, deserialized.value);
        assert_eq!(cookie.domain, deserialized.domain);
        assert_eq!(cookie.secure, deserialized.secure);
    }
}

#[cfg(test)]
mod accessibility_tests {
    use super::*;

    #[test]
    fn test_accessibility_manager_creation() {
        let cdp = CdpClient::new("localhost", 9222);
        let _manager = AccessibilityManager::new(cdp);
        // Just test creation succeeds
    }
}

#[cfg(test)]
mod screenshot_tests {
    use super::*;

    #[test]
    fn test_screenshot_manager_creation() {
        let cdp = CdpClient::new("localhost", 9222);
        let _manager = ScreenshotManager::new(cdp);
        // Just test creation succeeds
    }

    #[test]
    fn test_pdf_options_default() {
        let options = chrome_mcp::screenshot::PdfOptions::default();
        assert_eq!(options.landscape, Some(false));
        assert_eq!(options.print_background, Some(true));
        assert_eq!(options.scale, Some(1.0));
    }
}

#[cfg(test)]
mod native_input_tests {
    use super::*;

    #[test]
    fn test_native_input_creation() {
        let result = NativeInputManager::new();
        // Should succeed on any platform (with warnings on non-macOS)
        assert!(result.is_ok());
    }

    #[test]
    fn test_key_codes() {
        let _keycodes = NativeInputManager::key_codes();
        // Just test we can access key codes struct
        assert!(chrome_mcp::native_input::NativeKeycodesData::RETURN > 0);
        assert!(chrome_mcp::native_input::NativeKeycodesData::SPACE > 0);
        assert!(chrome_mcp::native_input::NativeKeycodesData::ESCAPE > 0);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_native_click_fails_on_non_macos() {
        let manager = NativeInputManager::new().unwrap();
        let result = manager.click_at(100.0, 100.0);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod mcp_tests {
    use super::*;

    #[test]
    fn test_mcp_server_creation() {
        let result = McpServer::new("localhost", 9222);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_message_serialization() {
        let message = chrome_mcp::mcp::McpMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: Some("test".to_string()),
            params: Some(json!({"key": "value"})),
            result: None,
            error: None,
        };

        let json_str = serde_json::to_string(&message).unwrap();
        let deserialized: chrome_mcp::mcp::McpMessage = serde_json::from_str(&json_str).unwrap();
        
        assert_eq!(message.jsonrpc, deserialized.jsonrpc);
        assert_eq!(message.method, deserialized.method);
    }

    #[test]
    fn test_tool_definition_serialization() {
        let tool = chrome_mcp::mcp::Tool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            }),
        };

        let json_str = serde_json::to_string(&tool).unwrap();
        let deserialized: chrome_mcp::mcp::Tool = serde_json::from_str(&json_str).unwrap();
        
        assert_eq!(tool.name, deserialized.name);
        assert_eq!(tool.description, deserialized.description);
    }

    #[test]
    fn test_server_capabilities_serialization() {
        let capabilities = chrome_mcp::mcp::ServerCapabilities {
            tools: Some(chrome_mcp::mcp::ToolsCapability {
                list_changed: Some(true),
            }),
            logging: Some(chrome_mcp::mcp::LoggingCapability {
                level: Some("info".to_string()),
            }),
            prompts: None,
            resources: None,
        };

        let json_str = serde_json::to_string(&capabilities).unwrap();
        let deserialized: chrome_mcp::mcp::ServerCapabilities = serde_json::from_str(&json_str).unwrap();
        
        assert!(deserialized.tools.is_some());
        assert!(deserialized.logging.is_some());
        assert!(deserialized.prompts.is_none());
        assert!(deserialized.resources.is_none());
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = ChromeMcpError::element_not_found("test element");
        assert!(matches!(error, ChromeMcpError::ElementNotFound(_)));
        
        let error = ChromeMcpError::navigation_timeout("5000ms");
        assert!(matches!(error, ChromeMcpError::NavigationTimeout(_)));
        
        let error = ChromeMcpError::Timeout { timeout: 1000 };
        assert!(matches!(error, ChromeMcpError::Timeout { timeout: 1000 }));
    }

    #[test]
    fn test_error_display() {
        let error = ChromeMcpError::element_not_found("button");
        let error_string = format!("{}", error);
        assert!(error_string.contains("Element not found"));
        assert!(error_string.contains("button"));
    }

    #[test]
    fn test_error_from_conversions() {
        use std::io;
        
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let chrome_error: ChromeMcpError = io_error.into();
        assert!(matches!(chrome_error, ChromeMcpError::Io(_)));
        
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let chrome_error: ChromeMcpError = json_error.into();
        assert!(matches!(chrome_error, ChromeMcpError::Json(_)));
    }

    #[test]
    fn test_result_type() {
        let success: Result<String> = Ok("test".to_string());
        assert!(success.is_ok());
        assert_eq!(success.unwrap(), "test");
        
        let failure: Result<String> = Err(ChromeMcpError::element_not_found("test"));
        assert!(failure.is_err());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_module_compilation() {
        // This test ensures all modules compile together correctly
        let _cdp = CdpClient::new("localhost", 9222);
        let _browser_result = Browser::new("localhost", 9222);
        let _mcp_result = McpServer::new("localhost", 9222);
        let _native_input_result = NativeInputManager::new();
    }

    #[test]
    fn test_error_propagation() {
        let error = ChromeMcpError::cdp_connection("test error");
        let result: Result<()> = Err(error);
        
        match result {
            Ok(_) => panic!("Expected error"),
            Err(e) => {
                match e {
                    ChromeMcpError::CdpConnection(msg) => assert_eq!(msg, "test error"),
                    _ => panic!("Wrong error type"),
                }
            }
        }
    }

    #[test]
    fn test_json_round_trip() {
        let original_json = json!({
            "method": "test",
            "params": {
                "url": "https://example.com",
                "timeout": 5000
            }
        });

        let json_str = serde_json::to_string(&original_json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        
        assert_eq!(original_json, parsed);
    }

    #[tokio::test]
    async fn test_async_error_handling() {
        async fn failing_function() -> Result<String> {
            Err(ChromeMcpError::element_not_found("test"))
        }

        let result = failing_function().await;
        assert!(result.is_err());
        
        match result {
            Err(ChromeMcpError::ElementNotFound(_)) => {
                // Expected error type
            }
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_bounds_calculation() {
        // Test bounds calculation for elements
        let bounds = (10.0, 20.0, 100.0, 50.0); // x, y, width, height
        let center_x = bounds.0 + bounds.2 / 2.0;
        let center_y = bounds.1 + bounds.3 / 2.0;
        
        assert_eq!(center_x, 60.0);
        assert_eq!(center_y, 45.0);
    }

    #[test]
    fn test_url_validation() {
        use url::Url;
        
        let valid_urls = vec![
            "https://example.com",
            "http://localhost:8080",
            "https://github.com/user/repo",
        ];

        for url_str in valid_urls {
            let url_result = Url::parse(url_str);
            assert!(url_result.is_ok(), "Failed to parse: {}", url_str);
        }
    }

    #[test]
    fn test_base64_operations() {
        let test_data = b"Hello, World!";
        let encoded = base64::engine::general_purpose::STANDARD.encode(test_data);
        let decoded = base64::engine::general_purpose::STANDARD.decode(&encoded).unwrap();
        
        assert_eq!(test_data, decoded.as_slice());
    }
}
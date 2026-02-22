use crate::browser::{Browser, Cookie, PdfOptions, WaitCondition};
use crate::error::{ChromeMcpError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
// use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

/// MCP Server implementation for Chrome automation
pub struct McpServer {
    browser: Browser,
    capabilities: ServerCapabilities,
}

/// MCP Server capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
    pub logging: Option<LoggingCapability>,
    pub prompts: Option<PromptsCapability>,
    pub resources: Option<ResourcesCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingCapability {
    pub level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
    pub subscribe: Option<bool>,
}

/// MCP Protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMessage {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: Option<String>,
    pub params: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<McpError>,
}

/// MCP Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(chrome_host: &str, chrome_port: u16) -> Result<Self> {
        let browser = Browser::new(chrome_host, chrome_port)?;
        let capabilities = ServerCapabilities {
            tools: Some(ToolsCapability {
                list_changed: Some(true),
            }),
            logging: Some(LoggingCapability {
                level: Some("info".to_string()),
            }),
            prompts: None,
            resources: None,
        };

        Ok(Self {
            browser,
            capabilities,
        })
    }

    /// Run the MCP server over stdio
    pub async fn run_stdio(&mut self) -> Result<()> {
        info!("Starting chrome-mcp server over stdio");

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut buffer = String::new();

        loop {
            buffer.clear();
            
            match reader.read_line(&mut buffer).await {
                Ok(0) => {
                    // EOF reached
                    info!("stdin closed, shutting down");
                    break;
                }
                Ok(_) => {
                    let line = buffer.trim();
                    if line.is_empty() {
                        continue;
                    }

                    debug!("Received: {}", line);

                    // Parse and handle the message
                    match self.handle_message(line).await {
                        Ok(response) => {
                            if let Some(resp) = response {
                                let response_json = serde_json::to_string(&resp)?;
                                debug!("Sending: {}", response_json);
                                
                                stdout.write_all(response_json.as_bytes()).await?;
                                stdout.write_all(b"\n").await?;
                                stdout.flush().await?;
                            }
                        }
                        Err(e) => {
                            error!("Error handling message: {}", e);
                            
                            // Send error response if we can parse the message ID
                            if let Ok(msg) = serde_json::from_str::<McpMessage>(line) {
                                let error_response = McpMessage {
                                    jsonrpc: "2.0".to_string(),
                                    id: msg.id,
                                    method: None,
                                    params: None,
                                    result: None,
                                    error: Some(McpError {
                                        code: -32603, // Internal error
                                        message: e.to_string(),
                                        data: None,
                                    }),
                                };

                                let error_json = serde_json::to_string(&error_response)?;
                                stdout.write_all(error_json.as_bytes()).await?;
                                stdout.write_all(b"\n").await?;
                                stdout.flush().await?;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle an incoming MCP message
    async fn handle_message(&mut self, message: &str) -> Result<Option<McpMessage>> {
        let msg: McpMessage = serde_json::from_str(message)
            .map_err(|e| ChromeMcpError::mcp_protocol_error(format!("Invalid JSON: {}", e)))?;

        match msg.method.as_deref() {
            Some("initialize") => self.handle_initialize(&msg).await,
            Some("tools/list") => self.handle_tools_list(&msg).await,
            Some("tools/call") => self.handle_tools_call(&msg).await,
            Some("ping") => self.handle_ping(&msg).await,
            Some(method) => {
                warn!("Unknown method: {}", method);
                Ok(Some(McpMessage {
                    jsonrpc: "2.0".to_string(),
                    id: msg.id,
                    method: None,
                    params: None,
                    result: None,
                    error: Some(McpError {
                        code: -32601, // Method not found
                        message: format!("Method not found: {}", method),
                        data: None,
                    }),
                }))
            }
            None => {
                // This might be a response to a request we sent
                debug!("Received response: {:?}", msg);
                Ok(None)
            }
        }
    }

    /// Handle initialize request
    async fn handle_initialize(&mut self, msg: &McpMessage) -> Result<Option<McpMessage>> {
        info!("Handling initialize request");

        // Connect to Chrome
        match self.browser.connect(None).await {
            Ok(tab_id) => {
                info!("Connected to Chrome tab: {}", tab_id);
            }
            Err(e) => {
                warn!("Failed to connect to Chrome: {}", e);
                // Continue anyway - connection can be retried
            }
        }

        Ok(Some(McpMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id.clone(),
            method: None,
            params: None,
            result: Some(json!({
                "protocolVersion": "1.0.0",
                "serverInfo": {
                    "name": "chrome-mcp",
                    "version": "0.1.0"
                },
                "capabilities": self.capabilities
            })),
            error: None,
        }))
    }

    /// Handle tools/list request
    async fn handle_tools_list(&self, msg: &McpMessage) -> Result<Option<McpMessage>> {
        debug!("Handling tools/list request");

        let tools = self.get_available_tools();

        Ok(Some(McpMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id.clone(),
            method: None,
            params: None,
            result: Some(json!({
                "tools": tools
            })),
            error: None,
        }))
    }

    /// Handle tools/call request
    async fn handle_tools_call(&mut self, msg: &McpMessage) -> Result<Option<McpMessage>> {
        let params = msg.params.as_ref()
            .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing params in tools/call"))?;

        let name = params.get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing tool name"))?;

        let default_args = json!({});
        let arguments = params.get("arguments").unwrap_or(&default_args);

        debug!("Calling tool: {} with args: {}", name, arguments);

        let result = self.call_tool(name, arguments).await;

        match result {
            Ok(tool_result) => {
                Ok(Some(McpMessage {
                    jsonrpc: "2.0".to_string(),
                    id: msg.id.clone(),
                    method: None,
                    params: None,
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": tool_result
                        }]
                    })),
                    error: None,
                }))
            }
            Err(e) => {
                Ok(Some(McpMessage {
                    jsonrpc: "2.0".to_string(),
                    id: msg.id.clone(),
                    method: None,
                    params: None,
                    result: None,
                    error: Some(McpError {
                        code: -32603,
                        message: format!("Tool execution failed: {}", e),
                        data: Some(json!({ "tool": name, "arguments": arguments })),
                    }),
                }))
            }
        }
    }

    /// Handle ping request
    async fn handle_ping(&self, msg: &McpMessage) -> Result<Option<McpMessage>> {
        Ok(Some(McpMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id.clone(),
            method: None,
            params: None,
            result: Some(json!({})),
            error: None,
        }))
    }

    /// Get list of available tools
    fn get_available_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "chrome_navigate".to_string(),
                description: "Navigate to a URL".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to navigate to"
                        }
                    },
                    "required": ["url"]
                }),
            },
            Tool {
                name: "chrome_click".to_string(),
                description: "Click on an element by CSS selector, text content, or accessibility label".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "CSS selector, text content, or accessibility label of element to click"
                        }
                    },
                    "required": ["target"]
                }),
            },
            Tool {
                name: "chrome_type".to_string(),
                description: "Type text into an element or the currently focused element".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Text to type"
                        },
                        "selector": {
                            "type": "string",
                            "description": "Optional CSS selector to focus first"
                        }
                    },
                    "required": ["text"]
                }),
            },
            Tool {
                name: "chrome_screenshot".to_string(),
                description: "Take a screenshot of the current page".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "format": {
                            "type": "string",
                            "description": "Image format: png or jpeg",
                            "enum": ["png", "jpeg"]
                        },
                        "quality": {
                            "type": "integer",
                            "description": "JPEG quality (1-100)",
                            "minimum": 1,
                            "maximum": 100
                        },
                        "full_page": {
                            "type": "boolean",
                            "description": "Capture full page or just viewport"
                        }
                    }
                }),
            },
            Tool {
                name: "chrome_evaluate".to_string(),
                description: "Execute JavaScript in the browser".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "javascript": {
                            "type": "string",
                            "description": "JavaScript code to execute"
                        }
                    },
                    "required": ["javascript"]
                }),
            },
            Tool {
                name: "chrome_tabs".to_string(),
                description: "List, create, or switch between browser tabs".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "description": "Action to perform",
                            "enum": ["list", "create", "switch", "close"]
                        },
                        "tab_id": {
                            "type": "string",
                            "description": "Tab ID (for switch/close actions)"
                        },
                        "url": {
                            "type": "string",
                            "description": "URL for new tab (create action)"
                        }
                    },
                    "required": ["action"]
                }),
            },
            Tool {
                name: "chrome_scroll".to_string(),
                description: "Scroll the page or scroll to an element".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "x": {
                            "type": "integer",
                            "description": "Horizontal scroll amount in pixels"
                        },
                        "y": {
                            "type": "integer",
                            "description": "Vertical scroll amount in pixels"
                        },
                        "selector": {
                            "type": "string",
                            "description": "CSS selector of element to scroll to"
                        }
                    }
                }),
            },
            Tool {
                name: "chrome_hover".to_string(),
                description: "Hover over an element".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "CSS selector or text of element to hover over"
                        }
                    },
                    "required": ["target"]
                }),
            },
            Tool {
                name: "chrome_select".to_string(),
                description: "Select an option from a dropdown".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "selector": {
                            "type": "string",
                            "description": "CSS selector of the select element"
                        },
                        "value": {
                            "type": "string",
                            "description": "Value of the option to select"
                        }
                    },
                    "required": ["selector", "value"]
                }),
            },
            Tool {
                name: "chrome_wait".to_string(),
                description: "Wait for a condition to be met".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "condition": {
                            "type": "string",
                            "description": "Condition type",
                            "enum": ["element_present", "element_visible", "element_clickable", "text_present", "url_matches", "page_load", "network_idle"]
                        },
                        "target": {
                            "type": "string",
                            "description": "Target for the condition (selector, text, URL pattern)"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Timeout in milliseconds",
                            "default": 10000
                        }
                    },
                    "required": ["condition"]
                }),
            },
            Tool {
                name: "chrome_cookies".to_string(),
                description: "Get, set, or clear browser cookies".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "description": "Cookie action",
                            "enum": ["get", "set", "clear"]
                        },
                        "name": {
                            "type": "string",
                            "description": "Cookie name (for set action)"
                        },
                        "value": {
                            "type": "string",
                            "description": "Cookie value (for set action)"
                        },
                        "domain": {
                            "type": "string",
                            "description": "Cookie domain (for set action)"
                        },
                        "path": {
                            "type": "string",
                            "description": "Cookie path (for set action)"
                        }
                    },
                    "required": ["action"]
                }),
            },
            Tool {
                name: "chrome_pdf".to_string(),
                description: "Generate a PDF of the current page".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "landscape": {
                            "type": "boolean",
                            "description": "Landscape orientation"
                        },
                        "print_background": {
                            "type": "boolean",
                            "description": "Include background graphics"
                        },
                        "scale": {
                            "type": "number",
                            "description": "Scale factor (0.1 to 2.0)"
                        }
                    }
                }),
            },
            Tool {
                name: "chrome_accessibility_tree".to_string(),
                description: "Get the accessibility tree of the current page".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "summary": {
                            "type": "boolean",
                            "description": "Return a text summary instead of full tree"
                        }
                    }
                }),
            },
            Tool {
                name: "chrome_native_click".to_string(),
                description: "Click at screen coordinates using native input (for browser chrome)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "x": {
                            "type": "number",
                            "description": "X coordinate on screen"
                        },
                        "y": {
                            "type": "number",
                            "description": "Y coordinate on screen"
                        }
                    },
                    "required": ["x", "y"]
                }),
            },
            Tool {
                name: "chrome_find".to_string(),
                description: "Find elements by text, role, or selector and return references".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query (text, role, or CSS selector)"
                        }
                    },
                    "required": ["query"]
                }),
            },
        ]
    }

    /// Execute a tool call
    async fn call_tool(&mut self, name: &str, arguments: &Value) -> Result<String> {
        match name {
            "chrome_navigate" => {
                let url = arguments.get("url")
                    .and_then(|u| u.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing url parameter"))?;
                
                self.browser.navigate(url).await?;
                Ok(format!("Navigated to: {}", url))
            }

            "chrome_click" => {
                let target = arguments.get("target")
                    .and_then(|t| t.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing target parameter"))?;
                
                self.browser.click(target).await?;
                Ok(format!("Clicked on: {}", target))
            }

            "chrome_type" => {
                let text = arguments.get("text")
                    .and_then(|t| t.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing text parameter"))?;
                
                let selector = arguments.get("selector").and_then(|s| s.as_str());
                
                self.browser.type_text(text, selector).await?;
                Ok(format!("Typed text: {}", text))
            }

            "chrome_screenshot" => {
                let format = arguments.get("format").and_then(|f| f.as_str());
                let quality = arguments.get("quality").and_then(|q| q.as_u64()).map(|q| q as u32);
                let full_page = arguments.get("full_page").and_then(|f| f.as_bool()).unwrap_or(false);
                
                let screenshot_data = if full_page {
                    self.browser.screenshot_full_page(format, quality).await?
                } else {
                    self.browser.screenshot(format, quality).await?
                };
                
                Ok(format!("data:image/{};base64,{}", format.unwrap_or("png"), screenshot_data))
            }

            "chrome_evaluate" => {
                let javascript = arguments.get("javascript")
                    .and_then(|j| j.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing javascript parameter"))?;
                
                let result = self.browser.evaluate(javascript).await?;
                Ok(serde_json::to_string_pretty(&result)?)
            }

            "chrome_tabs" => {
                let action = arguments.get("action")
                    .and_then(|a| a.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing action parameter"))?;
                
                match action {
                    "list" => {
                        let tabs = self.browser.list_tabs().await?;
                        Ok(serde_json::to_string_pretty(&tabs)?)
                    }
                    "create" => {
                        let url = arguments.get("url").and_then(|u| u.as_str());
                        let tab_id = self.browser.create_tab(url).await?;
                        Ok(format!("Created tab: {}", tab_id))
                    }
                    "switch" => {
                        let tab_id = arguments.get("tab_id")
                            .and_then(|t| t.as_str())
                            .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing tab_id parameter"))?;
                        
                        self.browser.switch_to_tab(tab_id).await?;
                        Ok(format!("Switched to tab: {}", tab_id))
                    }
                    "close" => {
                        let tab_id = arguments.get("tab_id")
                            .and_then(|t| t.as_str())
                            .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing tab_id parameter"))?;
                        
                        self.browser.close_tab(tab_id).await?;
                        Ok(format!("Closed tab: {}", tab_id))
                    }
                    _ => Err(ChromeMcpError::mcp_protocol_error(format!("Unknown tabs action: {}", action)))
                }
            }

            "chrome_scroll" => {
                if let Some(selector) = arguments.get("selector").and_then(|s| s.as_str()) {
                    self.browser.scroll_to_element(selector).await?;
                    Ok(format!("Scrolled to element: {}", selector))
                } else {
                    let x = arguments.get("x").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
                    let y = arguments.get("y").and_then(|y| y.as_i64()).unwrap_or(0) as i32;
                    
                    self.browser.scroll(x, y).await?;
                    Ok(format!("Scrolled by: ({}, {})", x, y))
                }
            }

            "chrome_hover" => {
                let target = arguments.get("target")
                    .and_then(|t| t.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing target parameter"))?;
                
                self.browser.hover(target).await?;
                Ok(format!("Hovered over: {}", target))
            }

            "chrome_select" => {
                let selector = arguments.get("selector")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing selector parameter"))?;
                
                let value = arguments.get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing value parameter"))?;
                
                self.browser.select_option(selector, value).await?;
                Ok(format!("Selected '{}' in {}", value, selector))
            }

            "chrome_wait" => {
                let condition_str = arguments.get("condition")
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing condition parameter"))?;
                
                let target = arguments.get("target").and_then(|t| t.as_str()).unwrap_or("");
                let timeout = arguments.get("timeout").and_then(|t| t.as_u64()).unwrap_or(10000);
                
                let condition = match condition_str {
                    "element_present" => WaitCondition::ElementPresent(target.to_string()),
                    "element_visible" => WaitCondition::ElementVisible(target.to_string()),
                    "element_clickable" => WaitCondition::ElementClickable(target.to_string()),
                    "text_present" => WaitCondition::TextPresent(target.to_string()),
                    "url_matches" => WaitCondition::UrlMatches(target.to_string()),
                    "page_load" => WaitCondition::PageLoad,
                    "network_idle" => WaitCondition::NetworkIdle(1000),
                    _ => return Err(ChromeMcpError::mcp_protocol_error(format!("Unknown condition: {}", condition_str)))
                };
                
                self.browser.wait_for_condition(condition, timeout).await?;
                Ok(format!("Wait condition '{}' satisfied", condition_str))
            }

            "chrome_cookies" => {
                let action = arguments.get("action")
                    .and_then(|a| a.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing action parameter"))?;
                
                match action {
                    "get" => {
                        let cookies = self.browser.get_cookies().await?;
                        Ok(serde_json::to_string_pretty(&cookies)?)
                    }
                    "set" => {
                        let name = arguments.get("name")
                            .and_then(|n| n.as_str())
                            .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing name parameter"))?;
                        
                        let value = arguments.get("value")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing value parameter"))?;
                        
                        let domain = arguments.get("domain")
                            .and_then(|d| d.as_str())
                            .unwrap_or("localhost");
                        
                        let path = arguments.get("path")
                            .and_then(|p| p.as_str())
                            .unwrap_or("/");
                        
                        let cookie = Cookie {
                            name: name.to_string(),
                            value: value.to_string(),
                            domain: domain.to_string(),
                            path: path.to_string(),
                            secure: false,
                            http_only: false,
                            same_site: None,
                            expires: None,
                        };
                        
                        self.browser.set_cookie(cookie).await?;
                        Ok(format!("Set cookie: {} = {}", name, value))
                    }
                    "clear" => {
                        self.browser.clear_cookies().await?;
                        Ok("Cleared all cookies".to_string())
                    }
                    _ => Err(ChromeMcpError::mcp_protocol_error(format!("Unknown cookies action: {}", action)))
                }
            }

            "chrome_pdf" => {
                let landscape = arguments.get("landscape").and_then(|l| l.as_bool());
                let print_background = arguments.get("print_background").and_then(|p| p.as_bool());
                let scale = arguments.get("scale").and_then(|s| s.as_f64());
                
                let options = if landscape.is_some() || print_background.is_some() || scale.is_some() {
                    Some(PdfOptions {
                        landscape,
                        print_background,
                        scale,
                        ..Default::default()
                    })
                } else {
                    None
                };
                
                let pdf_data = self.browser.pdf(options).await?;
                Ok(format!("data:application/pdf;base64,{}", pdf_data))
            }

            "chrome_accessibility_tree" => {
                let summary = arguments.get("summary").and_then(|s| s.as_bool()).unwrap_or(false);
                
                if summary {
                    let summary = self.browser.accessibility().get_tree_summary().await?;
                    Ok(summary.join("\n"))
                } else {
                    let tree = self.browser.accessibility_tree().await?;
                    Ok(serde_json::to_string_pretty(&tree)?)
                }
            }

            "chrome_native_click" => {
                let x = arguments.get("x")
                    .and_then(|x| x.as_f64())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing x parameter"))?;
                
                let y = arguments.get("y")
                    .and_then(|y| y.as_f64())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing y parameter"))?;
                
                self.browser.native_click(x, y).await?;
                Ok(format!("Native click at ({}, {})", x, y))
            }

            "chrome_find" => {
                let query = arguments.get("query")
                    .and_then(|q| q.as_str())
                    .ok_or_else(|| ChromeMcpError::mcp_protocol_error("Missing query parameter"))?;
                
                let elements = self.browser.find_elements(query).await?;
                Ok(serde_json::to_string_pretty(&elements)?)
            }

            _ => Err(ChromeMcpError::mcp_protocol_error(format!("Unknown tool: {}", name)))
        }
    }
}
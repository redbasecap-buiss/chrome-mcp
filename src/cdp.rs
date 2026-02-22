use crate::error::{ChromeMcpError, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, trace, warn};
use url::Url;
// use uuid::Uuid;

/// CDP message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpMessage {
    pub id: Option<u64>,
    pub method: Option<String>,
    pub params: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<CdpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

/// Chrome tab information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    pub description: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub websocket_debugger_url: Option<String>,
}

/// CDP client for communicating with Chrome DevTools
pub struct CdpClient {
    websocket: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    message_id: Arc<Mutex<u64>>,
    pending_requests: Arc<Mutex<HashMap<u64, mpsc::UnboundedSender<CdpMessage>>>>,
    event_sender: Option<mpsc::UnboundedSender<CdpMessage>>,
    chrome_host: String,
    chrome_port: u16,
    tab_id: Option<String>,
}

impl Clone for CdpClient {
    fn clone(&self) -> Self {
        Self {
            websocket: None, // WebSocket connections aren't cloneable, create new ones as needed
            message_id: Arc::clone(&self.message_id),
            pending_requests: Arc::clone(&self.pending_requests),
            event_sender: None,
            chrome_host: self.chrome_host.clone(),
            chrome_port: self.chrome_port,
            tab_id: self.tab_id.clone(),
        }
    }
}

impl CdpClient {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            websocket: None,
            message_id: Arc::new(Mutex::new(1)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            event_sender: None,
            chrome_host: host.to_string(),
            chrome_port: port,
            tab_id: None,
        }
    }

    /// List available tabs
    pub async fn list_tabs(&self) -> Result<Vec<TabInfo>> {
        let url = format!("http://{}:{}/json", self.chrome_host, self.chrome_port);
        debug!("Fetching tabs from: {}", url);

        let response = reqwest::get(&url)
            .await
            .map_err(|e| ChromeMcpError::cdp_connection(format!("Failed to fetch tabs: {}", e)))?;

        let tabs: Vec<TabInfo> = response
            .json()
            .await
            .map_err(|e| ChromeMcpError::cdp_protocol(format!("Failed to parse tab list: {}", e)))?;

        Ok(tabs)
    }

    /// Create a new tab
    pub async fn create_tab(&self, url: Option<&str>) -> Result<TabInfo> {
        let mut endpoint = format!("http://{}:{}/json/new", self.chrome_host, self.chrome_port);
        if let Some(u) = url {
            endpoint.push_str(&format!("?{}", u));
        }

        let response = reqwest::get(&endpoint)
            .await
            .map_err(|e| ChromeMcpError::cdp_connection(format!("Failed to create tab: {}", e)))?;

        let tab: TabInfo = response
            .json()
            .await
            .map_err(|e| ChromeMcpError::cdp_protocol(format!("Failed to parse new tab: {}", e)))?;

        Ok(tab)
    }

    /// Close a tab
    pub async fn close_tab(&self, tab_id: &str) -> Result<()> {
        let url = format!("http://{}:{}/json/close/{}", self.chrome_host, self.chrome_port, tab_id);
        
        let response = reqwest::get(&url)
            .await
            .map_err(|e| ChromeMcpError::cdp_connection(format!("Failed to close tab: {}", e)))?;

        if !response.status().is_success() {
            return Err(ChromeMcpError::cdp_protocol(format!("Failed to close tab: HTTP {}", response.status())));
        }

        Ok(())
    }

    /// Connect to a specific tab
    pub async fn connect_to_tab(&mut self, tab_id: &str) -> Result<()> {
        let tabs = self.list_tabs().await?;
        let tab = tabs
            .iter()
            .find(|t| t.id == tab_id)
            .ok_or_else(|| ChromeMcpError::tab_not_found(format!("Tab {} not found", tab_id)))?;

        let ws_url = tab
            .websocket_debugger_url
            .as_ref()
            .ok_or_else(|| ChromeMcpError::cdp_protocol("Tab has no WebSocket debugger URL".to_string()))?;

        debug!("Connecting to tab WebSocket: {}", ws_url);
        
        let url = Url::parse(ws_url)
            .map_err(|e| ChromeMcpError::cdp_connection(format!("Invalid WebSocket URL: {}", e)))?;

        let (ws_stream, _) = connect_async(url.as_str())
            .await
            .map_err(|e| ChromeMcpError::cdp_connection(format!("WebSocket connection failed: {}", e)))?;

        self.websocket = Some(ws_stream);
        self.tab_id = Some(tab_id.to_string());

        // Start message handling loop
        self.start_message_loop().await?;

        // Enable necessary CDP domains
        self.enable_domains().await?;

        Ok(())
    }

    /// Enable CDP domains required for automation
    async fn enable_domains(&mut self) -> Result<()> {
        let domains = vec![
            "Runtime",
            "Page",
            "DOM",
            "Input",
            "Network",
            "Accessibility",
        ];

        for domain in domains {
            self.send_command(&format!("{}.enable", domain), None).await?;
        }

        Ok(())
    }

    /// Start the message handling loop
    async fn start_message_loop(&mut self) -> Result<()> {
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        self.event_sender = Some(event_tx);

        if let Some(ws) = self.websocket.take() {
            let (_sink, mut stream) = ws.split();
            let pending_requests = Arc::clone(&self.pending_requests);

            // Spawn task to handle incoming messages
            tokio::spawn(async move {
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            trace!("Received CDP message: {}", text);
                            match serde_json::from_str::<CdpMessage>(&text) {
                                Ok(cdp_msg) => {
                                    if let Some(id) = cdp_msg.id {
                                        // This is a response to a request
                                        if let Some(sender) = pending_requests.lock().unwrap().remove(&id) {
                                            if sender.send(cdp_msg).is_err() {
                                                warn!("Failed to send response to waiting request {}", id);
                                            }
                                        }
                                    } else {
                                        // This is an event
                                        // For now, we'll just log events
                                        debug!("CDP Event: {:?}", cdp_msg);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse CDP message: {}", e);
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            warn!("WebSocket connection closed");
                            break;
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            });

            // Store the sink for sending messages
            // Note: In a real implementation, we'd need to store this properly
            // For now, we'll create a new connection when needed
        }

        Ok(())
    }

    /// Send a CDP command and wait for response
    pub async fn send_command(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = {
            let mut counter = self.message_id.lock().unwrap();
            let current = *counter;
            *counter += 1;
            current
        };

        let message = CdpMessage {
            id: Some(id),
            method: Some(method.to_string()),
            params,
            result: None,
            error: None,
        };

        let (response_tx, mut response_rx) = mpsc::unbounded_channel();
        self.pending_requests.lock().unwrap().insert(id, response_tx);

        // Send the message
        self.send_message(message).await?;

        // Wait for response with timeout
        let response = timeout(Duration::from_secs(30), response_rx.recv())
            .await
            .map_err(|_| ChromeMcpError::Timeout { timeout: 30000 })?
            .ok_or_else(|| ChromeMcpError::cdp_protocol("Response channel closed".to_string()))?;

        if let Some(error) = response.error {
            return Err(ChromeMcpError::cdp_protocol(format!(
                "CDP error {}: {}", error.code, error.message
            )));
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    /// Send a message to Chrome
    async fn send_message(&mut self, message: CdpMessage) -> Result<()> {
        // In a real implementation, we'd need to properly manage the WebSocket connection
        // For now, this is a simplified version
        
        // Create a new connection for each message (not efficient, but works for demo)
        if let Some(tab_id) = &self.tab_id {
            let tabs = self.list_tabs().await?;
            let tab = tabs
                .iter()
                .find(|t| t.id == *tab_id)
                .ok_or_else(|| ChromeMcpError::tab_not_found(format!("Tab {} not found", tab_id)))?;

            if let Some(ws_url) = &tab.websocket_debugger_url {
                let url = Url::parse(ws_url)?;
                let (mut ws_stream, _) = connect_async(url.as_str()).await?;

                let json_msg = serde_json::to_string(&message)?;
                trace!("Sending CDP message: {}", json_msg);
                
                ws_stream.send(Message::Text(json_msg)).await?;
                
                // Read the response
                if let Some(msg) = ws_stream.next().await {
                    if let Message::Text(text) = msg? {
                        let response: CdpMessage = serde_json::from_str(&text)?;
                        if let Some(sender) = self.pending_requests.lock().unwrap().remove(&message.id.unwrap_or(0)) {
                            let _ = sender.send(response);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Navigate to a URL
    pub async fn navigate(&mut self, url: &str) -> Result<Value> {
        self.send_command("Page.navigate", Some(json!({ "url": url }))).await
    }

    /// Evaluate JavaScript
    pub async fn evaluate_js(&mut self, expression: &str) -> Result<Value> {
        let result = self.send_command("Runtime.evaluate", Some(json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true
        }))).await?;

        if let Some(exception_details) = result.get("exceptionDetails") {
            return Err(ChromeMcpError::javascript_error(format!("JS Exception: {}", exception_details)));
        }

        Ok(result.get("result").unwrap_or(&Value::Null).clone())
    }

    /// Take a screenshot
    pub async fn screenshot(&mut self, format: Option<&str>, quality: Option<u32>) -> Result<String> {
        let mut params = json!({});
        
        if let Some(fmt) = format {
            params["format"] = json!(fmt);
        }
        
        if let Some(qual) = quality {
            params["quality"] = json!(qual);
        }

        let result = self.send_command("Page.captureScreenshot", Some(params)).await?;
        
        result
            .get("data")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ChromeMcpError::screenshot_error("No screenshot data returned"))
    }

    /// Click at coordinates
    pub async fn click_at(&mut self, x: f64, y: f64) -> Result<()> {
        // Mouse down
        self.send_command("Input.dispatchMouseEvent", Some(json!({
            "type": "mousePressed",
            "x": x,
            "y": y,
            "button": "left",
            "clickCount": 1
        }))).await?;

        // Small delay
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Mouse up
        self.send_command("Input.dispatchMouseEvent", Some(json!({
            "type": "mouseReleased",
            "x": x,
            "y": y,
            "button": "left",
            "clickCount": 1
        }))).await?;

        Ok(())
    }

    /// Type text
    pub async fn type_text(&mut self, text: &str) -> Result<()> {
        for ch in text.chars() {
            self.send_command("Input.dispatchKeyEvent", Some(json!({
                "type": "char",
                "text": ch.to_string()
            }))).await?;
            
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }

    /// Get accessibility tree
    pub async fn get_accessibility_tree(&mut self) -> Result<Value> {
        self.send_command("Accessibility.getFullAXTree", None).await
    }

    /// Find elements by selector
    pub async fn query_selector_all(&mut self, selector: &str) -> Result<Value> {
        // Get document root
        let doc_result = self.send_command("DOM.getDocument", None).await?;
        let root_node_id = doc_result
            .get("root")
            .and_then(|r| r.get("nodeId"))
            .and_then(|id| id.as_u64())
            .ok_or_else(|| ChromeMcpError::cdp_protocol("Could not get document root"))?;

        // Query for elements
        self.send_command("DOM.querySelectorAll", Some(json!({
            "nodeId": root_node_id,
            "selector": selector
        }))).await
    }

    /// Get the current tab ID
    pub fn current_tab_id(&self) -> Option<&str> {
        self.tab_id.as_deref()
    }
}

// We need to add reqwest dependency for HTTP requests to Chrome's REST API
// This is a placeholder - in the real implementation we'd add this to Cargo.toml
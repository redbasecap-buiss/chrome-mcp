use crate::accessibility::{AccessibilityManager, AccessibilityNode};
use crate::cdp::{CdpClient, TabInfo};
use crate::error::{ChromeMcpError, Result};
use crate::native_input::NativeInputManager;
use crate::screenshot::{ScreenshotManager};
pub use crate::screenshot::PdfOptions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing::{debug, info};

/// High-level browser automation interface
#[allow(dead_code)]
pub struct Browser {
    cdp: CdpClient,
    accessibility: AccessibilityManager,
    screenshot: ScreenshotManager,
    native_input: NativeInputManager,
    current_tab_id: Option<String>,
    network_events: Vec<NetworkEvent>,
    cookies: HashMap<String, Vec<Cookie>>,
}

/// Network event information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub timestamp: f64,
    pub status_code: Option<u32>,
    pub response_headers: Option<HashMap<String, String>>,
}

/// Cookie information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub expires: Option<f64>,
}

/// Element reference for consistent targeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRef {
    pub id: String,
    pub selector: Option<String>,
    pub accessibility_id: Option<String>,
    pub bounds: Option<(f64, f64, f64, f64)>, // x, y, width, height
    pub text: Option<String>,
    pub role: Option<String>,
}

/// Wait conditions
#[derive(Debug, Clone)]
pub enum WaitCondition {
    /// Wait for element to be present
    ElementPresent(String),
    /// Wait for element to be visible
    ElementVisible(String),
    /// Wait for element to be clickable
    ElementClickable(String),
    /// Wait for text to be present
    TextPresent(String),
    /// Wait for URL to match pattern
    UrlMatches(String),
    /// Wait for page load to complete
    PageLoad,
    /// Wait for network idle (no requests for specified duration)
    NetworkIdle(u64), // milliseconds
}

impl Browser {
    /// Create a new Browser instance
    pub fn new(chrome_host: &str, chrome_port: u16) -> Result<Self> {
        let cdp = CdpClient::new(chrome_host, chrome_port);
        let accessibility = AccessibilityManager::new(cdp.clone());
        let screenshot = ScreenshotManager::new(cdp.clone());
        let native_input = NativeInputManager::new()?;

        Ok(Self {
            cdp,
            accessibility,
            screenshot,
            native_input,
            current_tab_id: None,
            network_events: Vec::new(),
            cookies: HashMap::new(),
        })
    }

    /// Connect to Chrome and select a tab
    pub async fn connect(&mut self, tab_id: Option<&str>) -> Result<String> {
        info!("Connecting to Chrome browser");

        let tab = if let Some(id) = tab_id {
            // Connect to specific tab
            self.cdp.connect_to_tab(id).await?;
            id.to_string()
        } else {
            // Find an existing tab or create a new one
            let tabs = self.cdp.list_tabs().await?;
            let tab_id = if let Some(tab) = tabs.first() {
                tab.id.clone()
            } else {
                // Create new tab
                let new_tab = self.cdp.create_tab(None).await?;
                new_tab.id
            };

            self.cdp.connect_to_tab(&tab_id).await?;
            tab_id
        };

        self.current_tab_id = Some(tab.clone());
        info!("Connected to tab: {}", tab);
        Ok(tab)
    }

    /// List all available tabs
    pub async fn list_tabs(&self) -> Result<Vec<TabInfo>> {
        self.cdp.list_tabs().await
    }

    /// Create a new tab
    pub async fn create_tab(&mut self, url: Option<&str>) -> Result<String> {
        let tab = self.cdp.create_tab(url).await?;
        info!("Created new tab: {} ({})", tab.title, tab.id);
        Ok(tab.id)
    }

    /// Switch to a different tab
    pub async fn switch_to_tab(&mut self, tab_id: &str) -> Result<()> {
        self.cdp.connect_to_tab(tab_id).await?;
        self.current_tab_id = Some(tab_id.to_string());
        info!("Switched to tab: {}", tab_id);
        Ok(())
    }

    /// Close a tab
    pub async fn close_tab(&self, tab_id: &str) -> Result<()> {
        self.cdp.close_tab(tab_id).await?;
        info!("Closed tab: {}", tab_id);
        Ok(())
    }

    /// Navigate to a URL
    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        info!("Navigating to: {}", url);
        self.cdp.navigate(url).await?;
        
        // Wait for navigation to complete
        self.wait_for_condition(WaitCondition::PageLoad, 30000).await?;
        
        // Clear accessibility cache after navigation
        self.accessibility.clear_cache();
        
        Ok(())
    }

    /// Click on an element
    pub async fn click(&mut self, selector_or_text: &str) -> Result<()> {
        debug!("Attempting to click: {}", selector_or_text);

        // Try different strategies to find and click the element
        
        // Strategy 1: Try as CSS selector
        if let Ok(element_ref) = self.find_element_by_selector(selector_or_text).await {
            return self.click_element_ref(&element_ref).await;
        }

        // Strategy 2: Try as accessibility text
        if let Ok(element_ref) = self.find_element_by_text(selector_or_text).await {
            return self.click_element_ref(&element_ref).await;
        }

        // Strategy 3: Try as accessibility role
        if let Ok(element_ref) = self.find_element_by_role(selector_or_text).await {
            return self.click_element_ref(&element_ref).await;
        }

        Err(ChromeMcpError::element_not_found(format!(
            "Could not find element to click: {}", selector_or_text
        )))
    }

    /// Click at specific coordinates using native input
    pub async fn native_click(&self, x: f64, y: f64) -> Result<()> {
        info!("Native click at ({}, {})", x, y);
        self.native_input.click_at(x, y)
    }

    /// Type text into an element or the focused element
    pub async fn type_text(&mut self, text: &str, selector: Option<&str>) -> Result<()> {
        info!("Typing text: {}", text);

        if let Some(sel) = selector {
            // Click on the element first to focus it
            self.click(sel).await?;
            sleep(Duration::from_millis(100)).await;
        }

        // Type the text using CDP
        self.cdp.type_text(text).await?;
        
        Ok(())
    }

    /// Type text using native input
    pub async fn native_type(&self, text: &str) -> Result<()> {
        info!("Native typing: {}", text);
        self.native_input.type_text(text)
    }

    /// Take a screenshot
    pub async fn screenshot(&mut self, format: Option<&str>, quality: Option<u32>) -> Result<String> {
        let format = format.unwrap_or("png");
        self.screenshot.capture_with_options(format, quality, false).await
    }

    /// Take a full-page screenshot
    pub async fn screenshot_full_page(&mut self, format: Option<&str>, quality: Option<u32>) -> Result<String> {
        let format = format.unwrap_or("png");
        self.screenshot.capture_with_options(format, quality, true).await
    }

    /// Screenshot a specific element
    pub async fn screenshot_element(&mut self, selector: &str) -> Result<String> {
        self.screenshot.capture_element(selector).await
    }

    /// Evaluate JavaScript
    pub async fn evaluate(&mut self, javascript: &str) -> Result<Value> {
        debug!("Evaluating JavaScript: {}", javascript);
        self.cdp.evaluate_js(javascript).await
    }

    /// Scroll the page
    pub async fn scroll(&mut self, x: i32, y: i32) -> Result<()> {
        debug!("Scrolling by ({}, {})", x, y);
        self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": format!("window.scrollBy({}, {})", x, y)
        }))).await?;
        Ok(())
    }

    /// Scroll to element
    pub async fn scroll_to_element(&mut self, selector: &str) -> Result<()> {
        debug!("Scrolling to element: {}", selector);
        self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": format!(
                "document.querySelector('{}').scrollIntoView({{ behavior: 'smooth', block: 'center' }})", 
                selector.replace("'", "\\'")
            )
        }))).await?;
        Ok(())
    }

    /// Hover over an element
    pub async fn hover(&mut self, selector_or_text: &str) -> Result<()> {
        debug!("Hovering over: {}", selector_or_text);

        let element_ref = self.find_element_any_strategy(selector_or_text).await?;
        
        if let Some((x, y, _, _)) = element_ref.bounds {
            let center_x = x + element_ref.bounds.unwrap().2 / 2.0;
            let center_y = y + element_ref.bounds.unwrap().3 / 2.0;
            
            self.cdp.send_command("Input.dispatchMouseEvent", Some(json!({
                "type": "mouseMoved",
                "x": center_x,
                "y": center_y
            }))).await?;
        }

        Ok(())
    }

    /// Select option from dropdown
    pub async fn select_option(&mut self, selector: &str, option_value: &str) -> Result<()> {
        debug!("Selecting option '{}' in element: {}", option_value, selector);
        
        self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": format!(
                r#"
                const select = document.querySelector('{}');
                if (select) {{
                    select.value = '{}';
                    select.dispatchEvent(new Event('change', {{ bubbles: true }}));
                }} else {{
                    throw new Error('Select element not found');
                }}
                "#,
                selector.replace("'", "\\'"),
                option_value.replace("'", "\\'")
            )
        }))).await?;
        
        Ok(())
    }

    /// Wait for a condition to be met
    pub async fn wait_for_condition(&mut self, condition: WaitCondition, timeout_ms: u64) -> Result<()> {
        debug!("Waiting for condition: {:?} (timeout: {}ms)", condition, timeout_ms);

        let result = timeout(Duration::from_millis(timeout_ms), async {
            loop {
                match &condition {
                    WaitCondition::ElementPresent(selector) => {
                        if self.find_element_by_selector(selector).await.is_ok() {
                            break;
                        }
                    }
                    WaitCondition::ElementVisible(selector) => {
                        if self.is_element_visible(selector).await? {
                            break;
                        }
                    }
                    WaitCondition::ElementClickable(selector) => {
                        if self.is_element_clickable(selector).await? {
                            break;
                        }
                    }
                    WaitCondition::TextPresent(text) => {
                        if self.is_text_present(text).await? {
                            break;
                        }
                    }
                    WaitCondition::UrlMatches(pattern) => {
                        if self.current_url().await?.contains(pattern) {
                            break;
                        }
                    }
                    WaitCondition::PageLoad => {
                        let ready_state = self.cdp.send_command("Runtime.evaluate", Some(json!({
                            "expression": "document.readyState",
                            "returnByValue": true
                        }))).await?;
                        
                        if let Some(state) = ready_state.get("result").and_then(|r| r.get("value")).and_then(|v| v.as_str()) {
                            if state == "complete" {
                                break;
                            }
                        }
                    }
                    WaitCondition::NetworkIdle(idle_time) => {
                        // Simplified network idle detection
                        sleep(Duration::from_millis(*idle_time)).await;
                        break;
                    }
                }

                sleep(Duration::from_millis(100)).await;
            }
            Ok::<(), ChromeMcpError>(())
        }).await;

        match result {
            Ok(_) => {
                debug!("Wait condition satisfied");
                Ok(())
            }
            Err(_) => Err(ChromeMcpError::Timeout { timeout: timeout_ms }),
        }
    }

    /// Get current URL
    pub async fn current_url(&mut self) -> Result<String> {
        let result = self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": "window.location.href",
            "returnByValue": true
        }))).await?;

        result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ChromeMcpError::cdp_protocol("Could not get current URL"))
    }

    /// Get page title
    pub async fn page_title(&mut self) -> Result<String> {
        let result = self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": "document.title",
            "returnByValue": true
        }))).await?;

        result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ChromeMcpError::cdp_protocol("Could not get page title"))
    }

    /// Get accessibility tree
    pub async fn accessibility_tree(&mut self) -> Result<AccessibilityNode> {
        self.accessibility.get_full_tree().await
    }

    /// Get accessibility manager
    pub fn accessibility(&mut self) -> &mut AccessibilityManager {
        &mut self.accessibility
    }

    /// Find elements using various strategies
    pub async fn find_elements(&mut self, query: &str) -> Result<Vec<ElementRef>> {
        let mut results = Vec::new();

        // Try CSS selector
        if let Ok(element) = self.find_element_by_selector(query).await {
            results.push(element);
        }

        // Try accessibility text
        if let Ok(element) = self.find_element_by_text(query).await {
            results.push(element);
        }

        // Try accessibility role
        if let Ok(element) = self.find_element_by_role(query).await {
            results.push(element);
        }

        if results.is_empty() {
            return Err(ChromeMcpError::element_not_found(format!("No elements found for: {}", query)));
        }

        Ok(results)
    }

    /// Get cookies for current domain
    pub async fn get_cookies(&mut self) -> Result<Vec<Cookie>> {
        let result = self.cdp.send_command("Network.getCookies", None).await?;
        
        let cookies_json = result
            .get("cookies")
            .and_then(|c| c.as_array())
            .ok_or_else(|| ChromeMcpError::network_error("Invalid cookies response"))?;

        let cookies: Vec<Cookie> = cookies_json
            .iter()
            .filter_map(|cookie_json| {
                Some(Cookie {
                    name: cookie_json.get("name")?.as_str()?.to_string(),
                    value: cookie_json.get("value")?.as_str()?.to_string(),
                    domain: cookie_json.get("domain")?.as_str()?.to_string(),
                    path: cookie_json.get("path")?.as_str()?.to_string(),
                    secure: cookie_json.get("secure")?.as_bool().unwrap_or(false),
                    http_only: cookie_json.get("httpOnly")?.as_bool().unwrap_or(false),
                    same_site: cookie_json.get("sameSite").and_then(|s| s.as_str()).map(|s| s.to_string()),
                    expires: cookie_json.get("expires").and_then(|e| e.as_f64()),
                })
            })
            .collect();

        Ok(cookies)
    }

    /// Set a cookie
    pub async fn set_cookie(&mut self, cookie: Cookie) -> Result<()> {
        let mut params = json!({
            "name": cookie.name,
            "value": cookie.value,
            "domain": cookie.domain,
            "path": cookie.path,
            "secure": cookie.secure,
            "httpOnly": cookie.http_only,
        });

        if let Some(same_site) = cookie.same_site {
            params["sameSite"] = json!(same_site);
        }

        if let Some(expires) = cookie.expires {
            params["expires"] = json!(expires);
        }

        self.cdp.send_command("Network.setCookie", Some(params)).await?;
        Ok(())
    }

    /// Clear all cookies
    pub async fn clear_cookies(&mut self) -> Result<()> {
        self.cdp.send_command("Network.clearBrowserCookies", None).await?;
        Ok(())
    }

    /// Generate PDF of current page
    pub async fn pdf(&mut self, options: Option<PdfOptions>) -> Result<String> {
        self.screenshot.capture_pdf(options).await
    }

    // Private helper methods

    async fn find_element_any_strategy(&mut self, query: &str) -> Result<ElementRef> {
        // Try CSS selector first
        if let Ok(element) = self.find_element_by_selector(query).await {
            return Ok(element);
        }

        // Try accessibility text
        if let Ok(element) = self.find_element_by_text(query).await {
            return Ok(element);
        }

        // Try accessibility role
        if let Ok(element) = self.find_element_by_role(query).await {
            return Ok(element);
        }

        Err(ChromeMcpError::element_not_found(format!("Element not found: {}", query)))
    }

    async fn find_element_by_selector(&mut self, selector: &str) -> Result<ElementRef> {
        let nodes = self.cdp.query_selector_all(selector).await?;
        let node_ids = nodes
            .get("nodeIds")
            .and_then(|ids| ids.as_array())
            .ok_or_else(|| ChromeMcpError::element_not_found(format!("No elements found for selector: {}", selector)))?;

        if node_ids.is_empty() {
            return Err(ChromeMcpError::element_not_found(format!("No elements found for selector: {}", selector)));
        }

        // Use the first found element
        let node_id = node_ids[0]
            .as_u64()
            .ok_or_else(|| ChromeMcpError::cdp_protocol("Invalid node ID"))?;

        Ok(ElementRef {
            id: format!("dom-{}", node_id),
            selector: Some(selector.to_string()),
            accessibility_id: None,
            bounds: None, // TODO: Get bounds from DOM
            text: None,
            role: None,
        })
    }

    async fn find_element_by_text(&mut self, text: &str) -> Result<ElementRef> {
        let nodes = self.accessibility.find_clickable_by_text(text).await?;
        if let Some(node) = nodes.first() {
            Ok(ElementRef {
                id: format!("ax-{}", node.node_id),
                selector: None,
                accessibility_id: Some(node.node_id.clone()),
                bounds: node.bounds.as_ref().map(|b| (b.x, b.y, b.width, b.height)),
                text: node.name.clone(),
                role: node.role.clone(),
            })
        } else {
            Err(ChromeMcpError::element_not_found(format!("No clickable element found with text: {}", text)))
        }
    }

    async fn find_element_by_role(&mut self, role: &str) -> Result<ElementRef> {
        let nodes = self.accessibility.find_by_role(role).await?;
        if let Some(node) = nodes.first() {
            Ok(ElementRef {
                id: format!("ax-{}", node.node_id),
                selector: None,
                accessibility_id: Some(node.node_id.clone()),
                bounds: node.bounds.as_ref().map(|b| (b.x, b.y, b.width, b.height)),
                text: node.name.clone(),
                role: node.role.clone(),
            })
        } else {
            Err(ChromeMcpError::element_not_found(format!("No element found with role: {}", role)))
        }
    }

    async fn click_element_ref(&mut self, element_ref: &ElementRef) -> Result<()> {
        if let Some((x, y, width, height)) = element_ref.bounds {
            // Click at center of element
            let center_x = x + width / 2.0;
            let center_y = y + height / 2.0;
            self.cdp.click_at(center_x, center_y).await
        } else if let Some(ref selector) = element_ref.selector {
            // Try to click using JavaScript
            self.cdp.send_command("Runtime.evaluate", Some(json!({
                "expression": format!("document.querySelector('{}').click()", selector.replace("'", "\\'"))
            }))).await?;
            Ok(())
        } else {
            Err(ChromeMcpError::invalid_operation("Cannot click element: no bounds or selector"))
        }
    }

    async fn is_element_visible(&mut self, selector: &str) -> Result<bool> {
        let result = self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": format!(
                r#"
                const el = document.querySelector('{}');
                el && el.offsetParent !== null && 
                getComputedStyle(el).visibility !== 'hidden' && 
                getComputedStyle(el).display !== 'none'
                "#,
                selector.replace("'", "\\'")
            ),
            "returnByValue": true
        }))).await?;

        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    async fn is_element_clickable(&mut self, selector: &str) -> Result<bool> {
        let result = self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": format!(
                r#"
                const el = document.querySelector('{}');
                el && el.offsetParent !== null && 
                !el.disabled &&
                getComputedStyle(el).pointerEvents !== 'none'
                "#,
                selector.replace("'", "\\'")
            ),
            "returnByValue": true
        }))).await?;

        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    async fn is_text_present(&mut self, text: &str) -> Result<bool> {
        let result = self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": format!(
                "document.body.textContent.includes('{}')",
                text.replace("'", "\\'")
            ),
            "returnByValue": true
        }))).await?;

        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }
}
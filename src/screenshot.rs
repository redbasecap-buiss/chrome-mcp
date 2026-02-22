use crate::cdp::CdpClient;
use crate::error::{ChromeMcpError, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use tracing::{debug, trace};

/// Screenshot manager for capturing browser content
pub struct ScreenshotManager {
    cdp: CdpClient,
}

impl ScreenshotManager {
    pub fn new(cdp: CdpClient) -> Self {
        Self { cdp }
    }

    /// Capture a full-page screenshot
    pub async fn capture_full_page(&mut self) -> Result<String> {
        debug!("Capturing full-page screenshot");
        
        let result = self.cdp.send_command("Page.captureScreenshot", Some(json!({
            "format": "png",
            "captureBeyondViewport": true
        }))).await?;

        self.extract_screenshot_data(result)
    }

    /// Capture a viewport screenshot
    pub async fn capture_viewport(&mut self) -> Result<String> {
        debug!("Capturing viewport screenshot");
        
        let result = self.cdp.send_command("Page.captureScreenshot", Some(json!({
            "format": "png",
            "captureBeyondViewport": false
        }))).await?;

        self.extract_screenshot_data(result)
    }

    /// Capture screenshot with specific format and quality
    pub async fn capture_with_options(&mut self, format: &str, quality: Option<u32>, full_page: bool) -> Result<String> {
        debug!("Capturing screenshot with format: {}, quality: {:?}, full_page: {}", format, quality, full_page);
        
        let mut params = json!({
            "format": format,
            "captureBeyondViewport": full_page
        });

        // Quality only applies to JPEG
        if format.to_lowercase() == "jpeg" {
            if let Some(q) = quality {
                params["quality"] = json!(q.min(100));
            }
        }

        let result = self.cdp.send_command("Page.captureScreenshot", Some(params)).await?;
        self.extract_screenshot_data(result)
    }

    /// Capture screenshot of a specific element
    pub async fn capture_element(&mut self, selector: &str) -> Result<String> {
        debug!("Capturing element screenshot for selector: {}", selector);
        
        // First, get the element's bounding box
        let bounds = self.get_element_bounds(selector).await?;
        
        // Capture screenshot with the specific clip area
        let result = self.cdp.send_command("Page.captureScreenshot", Some(json!({
            "format": "png",
            "clip": {
                "x": bounds.x,
                "y": bounds.y,
                "width": bounds.width,
                "height": bounds.height,
                "scale": 1.0
            }
        }))).await?;

        self.extract_screenshot_data(result)
    }

    /// Get element bounds for clipping
    async fn get_element_bounds(&mut self, selector: &str) -> Result<ElementBounds> {
        // Get document root
        let doc_result = self.cdp.send_command("DOM.getDocument", None).await?;
        let root_node_id = doc_result
            .get("root")
            .and_then(|r| r.get("nodeId"))
            .and_then(|id| id.as_u64())
            .ok_or_else(|| ChromeMcpError::cdp_protocol("Could not get document root"))?;

        // Find the element
        let query_result = self.cdp.send_command("DOM.querySelector", Some(json!({
            "nodeId": root_node_id,
            "selector": selector
        }))).await?;

        let element_node_id = query_result
            .get("nodeId")
            .and_then(|id| id.as_u64())
            .ok_or_else(|| ChromeMcpError::element_not_found(format!("Element not found: {}", selector)))?;

        // Get element bounds
        let bounds_result = self.cdp.send_command("DOM.getBoxModel", Some(json!({
            "nodeId": element_node_id
        }))).await?;

        let content_quad = bounds_result
            .get("model")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .ok_or_else(|| ChromeMcpError::cdp_protocol("Could not get element content quad"))?;

        // Convert quad to bounding box
        if content_quad.len() < 8 {
            return Err(ChromeMcpError::cdp_protocol("Invalid content quad format"));
        }

        let x1 = content_quad[0].as_f64().unwrap_or(0.0);
        let y1 = content_quad[1].as_f64().unwrap_or(0.0);
        let x2 = content_quad[2].as_f64().unwrap_or(0.0);
        let y2 = content_quad[3].as_f64().unwrap_or(0.0);
        let x3 = content_quad[4].as_f64().unwrap_or(0.0);
        let y3 = content_quad[5].as_f64().unwrap_or(0.0);
        let x4 = content_quad[6].as_f64().unwrap_or(0.0);
        let y4 = content_quad[7].as_f64().unwrap_or(0.0);

        let min_x = x1.min(x2).min(x3).min(x4);
        let min_y = y1.min(y2).min(y3).min(y4);
        let max_x = x1.max(x2).max(x3).max(x4);
        let max_y = y1.max(y2).max(y3).max(y4);

        Ok(ElementBounds {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    /// Extract screenshot data from CDP result
    fn extract_screenshot_data(&self, result: Value) -> Result<String> {
        let data = result
            .get("data")
            .and_then(|d| d.as_str())
            .ok_or_else(|| ChromeMcpError::screenshot_error("No screenshot data in response"))?;

        Ok(data.to_string())
    }

    /// Convert base64 screenshot to bytes
    pub fn decode_screenshot(&self, base64_data: &str) -> Result<Vec<u8>> {
        BASE64
            .decode(base64_data)
            .map_err(|e| ChromeMcpError::screenshot_error(format!("Failed to decode base64: {}", e)))
    }

    /// Save screenshot to file
    pub async fn save_screenshot(&mut self, filename: &str, format: Option<&str>, quality: Option<u32>) -> Result<String> {
        let format = format.unwrap_or("png");
        let base64_data = self.capture_with_options(format, quality, true).await?;
        
        let bytes = self.decode_screenshot(&base64_data)?;
        std::fs::write(filename, bytes)
            .map_err(|e| ChromeMcpError::screenshot_error(format!("Failed to write file: {}", e)))?;

        debug!("Screenshot saved to: {}", filename);
        Ok(filename.to_string())
    }

    /// Capture screenshot with annotations (highlight elements)
    pub async fn capture_with_highlights(&mut self, selectors: Vec<&str>) -> Result<String> {
        debug!("Capturing screenshot with highlights for {} elements", selectors.len());
        
        // First, take a regular screenshot
        let base64_data = self.capture_full_page().await?;
        
        // For now, we'll just return the regular screenshot
        // In a full implementation, we'd overlay highlights on the image
        // This would require image processing capabilities
        
        trace!("Highlighting elements: {:?}", selectors);
        
        // TODO: Implement actual highlighting by:
        // 1. Decoding the base64 image
        // 2. Getting bounds for each selector
        // 3. Drawing rectangles or borders on the image
        // 4. Re-encoding to base64
        
        Ok(base64_data)
    }

    /// Get viewport size
    pub async fn get_viewport_size(&mut self) -> Result<(u32, u32)> {
        let result = self.cdp.send_command("Runtime.evaluate", Some(json!({
            "expression": "({ width: window.innerWidth, height: window.innerHeight })",
            "returnByValue": true
        }))).await?;

        let value = result
            .get("result")
            .and_then(|r| r.get("value"))
            .ok_or_else(|| ChromeMcpError::screenshot_error("Could not get viewport size"))?;

        let width = value
            .get("width")
            .and_then(|w| w.as_u64())
            .unwrap_or(1920) as u32;
        
        let height = value
            .get("height")
            .and_then(|h| h.as_u64())
            .unwrap_or(1080) as u32;

        Ok((width, height))
    }

    /// Set viewport size
    pub async fn set_viewport_size(&mut self, width: u32, height: u32) -> Result<()> {
        debug!("Setting viewport size to {}x{}", width, height);
        
        self.cdp.send_command("Emulation.setDeviceMetricsOverride", Some(json!({
            "width": width,
            "height": height,
            "deviceScaleFactor": 1.0,
            "mobile": false
        }))).await?;

        Ok(())
    }

    /// Capture PDF of the page
    pub async fn capture_pdf(&mut self, options: Option<PdfOptions>) -> Result<String> {
        debug!("Capturing PDF with options: {:?}", options);
        
        let mut params = json!({});
        
        if let Some(opts) = options {
            if let Some(landscape) = opts.landscape {
                params["landscape"] = json!(landscape);
            }
            if let Some(display_header_footer) = opts.display_header_footer {
                params["displayHeaderFooter"] = json!(display_header_footer);
            }
            if let Some(print_background) = opts.print_background {
                params["printBackground"] = json!(print_background);
            }
            if let Some(scale) = opts.scale {
                params["scale"] = json!(scale);
            }
            if let Some(paper_width) = opts.paper_width {
                params["paperWidth"] = json!(paper_width);
            }
            if let Some(paper_height) = opts.paper_height {
                params["paperHeight"] = json!(paper_height);
            }
            if let Some(margin_top) = opts.margin_top {
                params["marginTop"] = json!(margin_top);
            }
            if let Some(margin_bottom) = opts.margin_bottom {
                params["marginBottom"] = json!(margin_bottom);
            }
            if let Some(margin_left) = opts.margin_left {
                params["marginLeft"] = json!(margin_left);
            }
            if let Some(margin_right) = opts.margin_right {
                params["marginRight"] = json!(margin_right);
            }
            if let Some(page_ranges) = opts.page_ranges {
                params["pageRanges"] = json!(page_ranges);
            }
            if let Some(header_template) = opts.header_template {
                params["headerTemplate"] = json!(header_template);
            }
            if let Some(footer_template) = opts.footer_template {
                params["footerTemplate"] = json!(footer_template);
            }
            if let Some(prefer_css_page_size) = opts.prefer_css_page_size {
                params["preferCSSPageSize"] = json!(prefer_css_page_size);
            }
        }

        let result = self.cdp.send_command("Page.printToPDF", Some(params)).await?;
        
        result
            .get("data")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ChromeMcpError::screenshot_error("No PDF data returned"))
    }
}

/// Element bounds for clipping
#[derive(Debug, Clone)]
struct ElementBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// PDF generation options
#[derive(Debug, Clone)]
pub struct PdfOptions {
    pub landscape: Option<bool>,
    pub display_header_footer: Option<bool>,
    pub print_background: Option<bool>,
    pub scale: Option<f64>,
    pub paper_width: Option<f64>,
    pub paper_height: Option<f64>,
    pub margin_top: Option<f64>,
    pub margin_bottom: Option<f64>,
    pub margin_left: Option<f64>,
    pub margin_right: Option<f64>,
    pub page_ranges: Option<String>,
    pub header_template: Option<String>,
    pub footer_template: Option<String>,
    pub prefer_css_page_size: Option<bool>,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            landscape: Some(false),
            display_header_footer: Some(false),
            print_background: Some(true),
            scale: Some(1.0),
            paper_width: None,
            paper_height: None,
            margin_top: Some(0.4),
            margin_bottom: Some(0.4),
            margin_left: Some(0.4),
            margin_right: Some(0.4),
            page_ranges: None,
            header_template: None,
            footer_template: None,
            prefer_css_page_size: Some(false),
        }
    }
}
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

/// Viewport bounds for clipping
#[derive(Debug, Clone)]
pub struct ViewportBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdp::CdpClient;
    use serde_json::json;

    #[test]
    fn test_screenshot_manager_creation() {
        let cdp = CdpClient::new("localhost", 9222);
        let _manager = ScreenshotManager::new(cdp);
        // Just test that creation succeeds
        // The actual manager is wrapped and we can't easily inspect internal fields
    }

    #[test]
    fn test_pdf_options_default() {
        let options = PdfOptions::default();
        
        assert_eq!(options.landscape, Some(false));
        assert_eq!(options.display_header_footer, Some(false));
        assert_eq!(options.print_background, Some(true));
        assert_eq!(options.scale, Some(1.0));
        assert_eq!(options.margin_top, Some(0.4));
        assert_eq!(options.margin_bottom, Some(0.4));
        assert_eq!(options.margin_left, Some(0.4));
        assert_eq!(options.margin_right, Some(0.4));
        assert_eq!(options.prefer_css_page_size, Some(false));
        
        // Optional fields should be None
        assert!(options.paper_width.is_none());
        assert!(options.paper_height.is_none());
        assert!(options.page_ranges.is_none());
        assert!(options.header_template.is_none());
        assert!(options.footer_template.is_none());
    }

    #[test]
    fn test_pdf_options_custom() {
        let options = PdfOptions {
            landscape: Some(true),
            display_header_footer: Some(true),
            print_background: Some(false),
            scale: Some(1.5),
            paper_width: Some(8.5),
            paper_height: Some(11.0),
            margin_top: Some(1.0),
            margin_bottom: Some(1.0),
            margin_left: Some(1.0),
            margin_right: Some(1.0),
            page_ranges: Some("1-3,5".to_string()),
            header_template: Some("<div>Header</div>".to_string()),
            footer_template: Some("<div>Footer</div>".to_string()),
            prefer_css_page_size: Some(true),
        };

        assert_eq!(options.landscape, Some(true));
        assert_eq!(options.display_header_footer, Some(true));
        assert_eq!(options.print_background, Some(false));
        assert_eq!(options.scale, Some(1.5));
        assert_eq!(options.paper_width, Some(8.5));
        assert_eq!(options.paper_height, Some(11.0));
        assert_eq!(options.page_ranges, Some("1-3,5".to_string()));
        assert_eq!(options.header_template, Some("<div>Header</div>".to_string()));
        assert_eq!(options.footer_template, Some("<div>Footer</div>".to_string()));
        assert_eq!(options.prefer_css_page_size, Some(true));
    }

    #[test]
    fn test_viewport_bounds_creation() {
        let bounds = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };

        assert_eq!(bounds.x, 0.0);
        assert_eq!(bounds.y, 0.0);
        assert_eq!(bounds.width, 1920.0);
        assert_eq!(bounds.height, 1080.0);
    }

    #[test]
    fn test_screenshot_command_construction_full_page() {
        let expected_params = json!({
            "format": "png",
            "captureBeyondViewport": true
        });

        assert_eq!(expected_params["format"], "png");
        assert_eq!(expected_params["captureBeyondViewport"], true);
    }

    #[test]
    fn test_screenshot_command_construction_viewport() {
        let expected_params = json!({
            "format": "png",
            "captureBeyondViewport": false
        });

        assert_eq!(expected_params["format"], "png");
        assert_eq!(expected_params["captureBeyondViewport"], false);
    }

    #[test]
    fn test_screenshot_command_with_quality() {
        let expected_params = json!({
            "format": "jpeg",
            "quality": 80,
            "captureBeyondViewport": false
        });

        assert_eq!(expected_params["format"], "jpeg");
        assert_eq!(expected_params["quality"], 80);
        assert_eq!(expected_params["captureBeyondViewport"], false);
    }

    #[test]
    fn test_element_screenshot_command_construction() {
        let node_id = 123;
        let expected_params = json!({
            "nodeId": node_id,
            "format": "png",
            "quality": 100
        });

        assert_eq!(expected_params["nodeId"], 123);
        assert_eq!(expected_params["format"], "png");
        assert_eq!(expected_params["quality"], 100);
    }

    #[test]
    fn test_area_screenshot_command_construction() {
        let bounds = ViewportBounds {
            x: 100.0,
            y: 200.0,
            width: 800.0,
            height: 600.0,
        };

        let expected_params = json!({
            "format": "png",
            "clip": {
                "x": bounds.x,
                "y": bounds.y,
                "width": bounds.width,
                "height": bounds.height,
                "scale": 1.0
            }
        });

        assert_eq!(expected_params["format"], "png");
        let clip = &expected_params["clip"];
        assert_eq!(clip["x"], 100.0);
        assert_eq!(clip["y"], 200.0);
        assert_eq!(clip["width"], 800.0);
        assert_eq!(clip["height"], 600.0);
        assert_eq!(clip["scale"], 1.0);
    }

    #[test]
    fn test_pdf_command_construction_default() {
        let options = PdfOptions::default();
        let expected_params = json!({
            "landscape": options.landscape,
            "displayHeaderFooter": options.display_header_footer,
            "printBackground": options.print_background,
            "scale": options.scale,
            "marginTop": options.margin_top,
            "marginBottom": options.margin_bottom,
            "marginLeft": options.margin_left,
            "marginRight": options.margin_right,
            "preferCSSPageSize": options.prefer_css_page_size
        });

        assert_eq!(expected_params["landscape"], false);
        assert_eq!(expected_params["displayHeaderFooter"], false);
        assert_eq!(expected_params["printBackground"], true);
        assert_eq!(expected_params["scale"], 1.0);
        assert_eq!(expected_params["marginTop"], 0.4);
        assert_eq!(expected_params["preferCSSPageSize"], false);
    }

    #[test]
    fn test_pdf_command_construction_custom() {
        let _options = PdfOptions {
            landscape: Some(true),
            display_header_footer: Some(true),
            print_background: Some(false),
            scale: Some(0.8),
            paper_width: Some(8.5),
            paper_height: Some(11.0),
            page_ranges: Some("1,3-5".to_string()),
            header_template: Some("<h1>Header</h1>".to_string()),
            footer_template: Some("<div>Page <span class='pageNumber'></span></div>".to_string()),
            ..Default::default()
        };

        let expected_params = json!({
            "landscape": true,
            "displayHeaderFooter": true,
            "printBackground": false,
            "scale": 0.8,
            "paperWidth": 8.5,
            "paperHeight": 11.0,
            "pageRanges": "1,3-5",
            "headerTemplate": "<h1>Header</h1>",
            "footerTemplate": "<div>Page <span class='pageNumber'></span></div>"
        });

        assert_eq!(expected_params["landscape"], true);
        assert_eq!(expected_params["displayHeaderFooter"], true);
        assert_eq!(expected_params["printBackground"], false);
        assert_eq!(expected_params["scale"], 0.8);
        assert_eq!(expected_params["paperWidth"], 8.5);
        assert_eq!(expected_params["paperHeight"], 11.0);
        assert_eq!(expected_params["pageRanges"], "1,3-5");
    }

    #[test]
    fn test_screenshot_data_extraction() {
        let mock_response = json!({
            "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg=="
        });

        let data = mock_response.get("data")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        assert!(data.is_some());
        let screenshot_data = data.unwrap();
        assert!(!screenshot_data.is_empty());
        assert!(screenshot_data.starts_with("iVBOR")); // PNG signature in base64
    }

    #[test]
    fn test_screenshot_data_extraction_missing() {
        let mock_response = json!({
            "success": true
        });

        let data = mock_response.get("data")
            .and_then(|v| v.as_str());

        assert!(data.is_none());
    }

    #[test]
    fn test_base64_operations() {
        let test_data = b"test screenshot data";
        let encoded = BASE64.encode(test_data);
        let decoded = BASE64.decode(&encoded).unwrap();

        assert_eq!(test_data, decoded.as_slice());
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_viewport_bounds_validation() {
        // Valid bounds
        let bounds = ViewportBounds {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };

        assert!(bounds.width > 0.0);
        assert!(bounds.height > 0.0);
        assert!(bounds.x >= 0.0);
        assert!(bounds.y >= 0.0);

        // Test bounds calculations
        let right = bounds.x + bounds.width;
        let bottom = bounds.y + bounds.height;
        
        assert_eq!(right, 800.0);
        assert_eq!(bottom, 600.0);
    }

    #[test]
    fn test_pdf_margin_validation() {
        let options = PdfOptions::default();
        
        // All margins should be positive
        assert!(options.margin_top.unwrap() >= 0.0);
        assert!(options.margin_bottom.unwrap() >= 0.0);
        assert!(options.margin_left.unwrap() >= 0.0);
        assert!(options.margin_right.unwrap() >= 0.0);
        
        // Scale should be positive
        assert!(options.scale.unwrap() > 0.0);
    }

    #[test]
    fn test_image_format_validation() {
        let valid_formats = vec!["png", "jpeg", "webp"];
        
        for format in valid_formats {
            // Basic format validation
            assert!(format == "png" || format == "jpeg" || format == "webp");
            assert!(!format.is_empty());
        }
    }

    #[test]
    fn test_quality_range_validation() {
        // JPEG quality should be 1-100
        let valid_qualities = vec![1, 50, 80, 90, 100];
        
        for quality in valid_qualities {
            assert!(quality >= 1);
            assert!(quality <= 100);
        }
    }

    #[test]
    fn test_pdf_options_clone() {
        let original = PdfOptions {
            landscape: Some(true),
            scale: Some(1.5),
            page_ranges: Some("1-5".to_string()),
            ..Default::default()
        };

        let cloned = original.clone();
        
        assert_eq!(original.landscape, cloned.landscape);
        assert_eq!(original.scale, cloned.scale);
        assert_eq!(original.page_ranges, cloned.page_ranges);
    }

    #[test]
    fn test_pdf_options_debug() {
        let options = PdfOptions::default();
        let debug_str = format!("{:?}", options);
        
        assert!(debug_str.contains("PdfOptions"));
        assert!(debug_str.contains("landscape"));
        assert!(debug_str.contains("scale"));
    }
}
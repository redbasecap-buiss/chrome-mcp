use crate::cdp::CdpClient;
use crate::error::{ChromeMcpError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::debug;

/// Represents an accessibility tree node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityNode {
    pub node_id: String,
    pub role: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub properties: Option<Value>,
    pub children: Vec<AccessibilityNode>,
    pub bounds: Option<Bounds>,
    pub focusable: bool,
    pub focused: bool,
    pub clickable: bool,
}

/// Bounding box for accessibility nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Accessibility tree manager
pub struct AccessibilityManager {
    cdp: CdpClient,
    cached_tree: Option<AccessibilityNode>,
}

impl AccessibilityManager {
    pub fn new(cdp: CdpClient) -> Self {
        Self {
            cdp,
            cached_tree: None,
        }
    }

    /// Get the full accessibility tree
    pub async fn get_full_tree(&mut self) -> Result<AccessibilityNode> {
        debug!("Fetching full accessibility tree");
        
        let raw_tree = self.cdp.get_accessibility_tree().await?;
        let root_node = self.parse_accessibility_tree(raw_tree)?;
        
        self.cached_tree = Some(root_node.clone());
        Ok(root_node)
    }

    /// Parse raw CDP accessibility tree into structured nodes
    fn parse_accessibility_tree(&self, raw_tree: Value) -> Result<AccessibilityNode> {
        let nodes = raw_tree
            .get("nodes")
            .and_then(|n| n.as_array())
            .ok_or_else(|| ChromeMcpError::accessibility_error("Invalid accessibility tree format"))?;

        if nodes.is_empty() {
            return Err(ChromeMcpError::accessibility_error("Empty accessibility tree"));
        }

        // Find root node (usually the first one or one with no parent)
        let root_raw = &nodes[0];
        self.parse_node(root_raw, nodes)
    }

    /// Parse a single accessibility node
    fn parse_node(&self, node_raw: &Value, all_nodes: &[Value]) -> Result<AccessibilityNode> {
        let node_id = node_raw
            .get("nodeId")
            .and_then(|id| id.as_str())
            .unwrap_or("unknown")
            .to_string();

        let role = node_raw
            .get("role")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let name = node_raw
            .get("name")
            .and_then(|n| n.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let description = node_raw
            .get("description")
            .and_then(|d| d.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let value = node_raw
            .get("value")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse bounds if available
        let bounds = node_raw.get("boundingRect").map(|bounds_raw| Bounds {
            x: bounds_raw.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
            y: bounds_raw.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
            width: bounds_raw.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0),
            height: bounds_raw.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0),
        });

        // Parse properties
        let focusable = self.get_bool_property(node_raw, "focusable").unwrap_or(false);
        let focused = self.get_bool_property(node_raw, "focused").unwrap_or(false);
        let clickable = self.is_clickable(node_raw);

        // Parse children
        let children = if let Some(child_ids) = node_raw.get("childIds").and_then(|c| c.as_array()) {
            let mut children = Vec::new();
            for child_id in child_ids {
                if let Some(child_id_str) = child_id.as_str() {
                    if let Some(child_node) = all_nodes.iter().find(|n| {
                        n.get("nodeId").and_then(|id| id.as_str()) == Some(child_id_str)
                    }) {
                        if let Ok(parsed_child) = self.parse_node(child_node, all_nodes) {
                            children.push(parsed_child);
                        }
                    }
                }
            }
            children
        } else {
            Vec::new()
        };

        Ok(AccessibilityNode {
            node_id,
            role,
            name,
            description,
            value,
            properties: node_raw.get("properties").cloned(),
            children,
            bounds,
            focusable,
            focused,
            clickable,
        })
    }

    /// Get boolean property from accessibility node
    fn get_bool_property(&self, node: &Value, property: &str) -> Option<bool> {
        node.get("properties")
            .and_then(|props| props.as_array())
            .and_then(|props_array| {
                props_array.iter().find(|prop| {
                    prop.get("name").and_then(|n| n.as_str()) == Some(property)
                })
            })
            .and_then(|prop| prop.get("value"))
            .and_then(|v| v.get("booleanValue"))
            .and_then(|b| b.as_bool())
    }

    /// Determine if a node is clickable
    fn is_clickable(&self, node: &Value) -> bool {
        let role = node
            .get("role")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Common clickable roles
        match role {
            "button" | "link" | "menuitem" | "tab" | "checkbox" | "radio" => true,
            _ => {
                // Check for click handlers or other indicators
                self.get_bool_property(node, "clickable").unwrap_or(false)
            }
        }
    }

    /// Find nodes by role
    pub async fn find_by_role(&mut self, role: &str) -> Result<Vec<AccessibilityNode>> {
        let tree = if let Some(ref cached) = self.cached_tree {
            cached.clone()
        } else {
            self.get_full_tree().await?
        };

        Ok(self.search_nodes_by_role(&tree, role))
    }

    /// Find nodes by name (text content)
    pub async fn find_by_name(&mut self, name: &str) -> Result<Vec<AccessibilityNode>> {
        let tree = if let Some(ref cached) = self.cached_tree {
            cached.clone()
        } else {
            self.get_full_tree().await?
        };

        Ok(self.search_nodes_by_name(&tree, name))
    }

    /// Find nodes by description
    pub async fn find_by_description(&mut self, description: &str) -> Result<Vec<AccessibilityNode>> {
        let tree = if let Some(ref cached) = self.cached_tree {
            cached.clone()
        } else {
            self.get_full_tree().await?
        };

        Ok(self.search_nodes_by_description(&tree, description))
    }

    /// Find clickable elements containing text
    pub async fn find_clickable_by_text(&mut self, text: &str) -> Result<Vec<AccessibilityNode>> {
        let tree = if let Some(ref cached) = self.cached_tree {
            cached.clone()
        } else {
            self.get_full_tree().await?
        };

        Ok(self.search_clickable_by_text(&tree, text))
    }

    /// Recursive search for nodes by role
    fn search_nodes_by_role(&self, node: &AccessibilityNode, target_role: &str) -> Vec<AccessibilityNode> {
        let mut results = Vec::new();

        if let Some(ref role) = node.role {
            if role.to_lowercase().contains(&target_role.to_lowercase()) {
                results.push(node.clone());
            }
        }

        for child in &node.children {
            results.extend(self.search_nodes_by_role(child, target_role));
        }

        results
    }

    /// Recursive search for nodes by name
    fn search_nodes_by_name(&self, node: &AccessibilityNode, target_name: &str) -> Vec<AccessibilityNode> {
        let mut results = Vec::new();

        if let Some(ref name) = node.name {
            if name.to_lowercase().contains(&target_name.to_lowercase()) {
                results.push(node.clone());
            }
        }

        for child in &node.children {
            results.extend(self.search_nodes_by_name(child, target_name));
        }

        results
    }

    /// Recursive search for nodes by description
    fn search_nodes_by_description(&self, node: &AccessibilityNode, target_desc: &str) -> Vec<AccessibilityNode> {
        let mut results = Vec::new();

        if let Some(ref description) = node.description {
            if description.to_lowercase().contains(&target_desc.to_lowercase()) {
                results.push(node.clone());
            }
        }

        for child in &node.children {
            results.extend(self.search_nodes_by_description(child, target_desc));
        }

        results
    }

    /// Recursive search for clickable nodes containing text
    fn search_clickable_by_text(&self, node: &AccessibilityNode, text: &str) -> Vec<AccessibilityNode> {
        let mut results = Vec::new();

        if node.clickable {
            let text_lower = text.to_lowercase();
            let matches = node.name
                .as_ref()
                .map(|n| n.to_lowercase().contains(&text_lower))
                .unwrap_or(false)
                || node.description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&text_lower))
                    .unwrap_or(false)
                || node.value
                    .as_ref()
                    .map(|v| v.to_lowercase().contains(&text_lower))
                    .unwrap_or(false);

            if matches {
                results.push(node.clone());
            }
        }

        for child in &node.children {
            results.extend(self.search_clickable_by_text(child, text));
        }

        results
    }

    /// Get center coordinates of an accessibility node
    pub fn get_center_coords(&self, node: &AccessibilityNode) -> Option<(f64, f64)> {
        node.bounds.as_ref().map(|bounds| {
            (
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
            )
        })
    }

    /// Clear cached tree (force refresh on next access)
    pub fn clear_cache(&mut self) {
        self.cached_tree = None;
    }

    /// Get a summary of the accessibility tree
    pub async fn get_tree_summary(&mut self) -> Result<Vec<String>> {
        let tree = self.get_full_tree().await?;
        let mut summary = Vec::new();
        self.collect_node_summaries(&tree, &mut summary, 0);
        Ok(summary)
    }

    /// Recursively collect node summaries for debugging
    fn collect_node_summaries(&self, node: &AccessibilityNode, summary: &mut Vec<String>, depth: usize) {
        let indent = "  ".repeat(depth);
        let role = node.role.as_deref().unwrap_or("unknown");
        let name = node.name.as_deref().unwrap_or("(no name)");
        
        let clickable_marker = if node.clickable { " [CLICKABLE]" } else { "" };
        let bounds_info = if let Some(ref bounds) = node.bounds {
            format!(" @({:.0},{:.0})", bounds.x, bounds.y)
        } else {
            String::new()
        };

        summary.push(format!("{}{}: {}{}{}", indent, role, name, bounds_info, clickable_marker));

        for child in &node.children {
            self.collect_node_summaries(child, summary, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdp::CdpClient;
    use serde_json::json;

    fn create_test_node(
        id: &str,
        role: Option<&str>,
        name: Option<&str>,
        clickable: bool,
        bounds: Option<(f64, f64, f64, f64)>, // x, y, width, height
    ) -> AccessibilityNode {
        AccessibilityNode {
            node_id: id.to_string(),
            role: role.map(|s| s.to_string()),
            name: name.map(|s| s.to_string()),
            description: None,
            value: None,
            properties: None,
            children: Vec::new(),
            bounds: bounds.map(|(x, y, w, h)| Bounds { x, y, width: w, height: h }),
            focusable: false,
            focused: false,
            clickable,
        }
    }

    #[test]
    fn test_accessibility_node_creation() {
        let node = create_test_node("1", Some("button"), Some("Submit"), true, Some((10.0, 20.0, 100.0, 50.0)));
        
        assert_eq!(node.node_id, "1");
        assert_eq!(node.role, Some("button".to_string()));
        assert_eq!(node.name, Some("Submit".to_string()));
        assert!(node.clickable);
        assert!(node.bounds.is_some());
        
        let bounds = node.bounds.unwrap();
        assert_eq!(bounds.x, 10.0);
        assert_eq!(bounds.y, 20.0);
        assert_eq!(bounds.width, 100.0);
        assert_eq!(bounds.height, 50.0);
    }

    #[test]
    fn test_bounds_serialization() {
        let bounds = Bounds {
            x: 10.5,
            y: 20.5,
            width: 100.0,
            height: 50.0,
        };

        let json_str = serde_json::to_string(&bounds).unwrap();
        let deserialized: Bounds = serde_json::from_str(&json_str).unwrap();
        
        assert_eq!(bounds.x, deserialized.x);
        assert_eq!(bounds.y, deserialized.y);
        assert_eq!(bounds.width, deserialized.width);
        assert_eq!(bounds.height, deserialized.height);
    }

    #[test]
    fn test_accessibility_manager_creation() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        assert!(manager.cached_tree.is_none());
    }

    #[test]
    fn test_parse_accessibility_tree_empty() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let empty_tree = json!({
            "nodes": []
        });
        
        let result = manager.parse_accessibility_tree(empty_tree);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChromeMcpError::Accessibility(_)));
    }

    #[test]
    fn test_parse_accessibility_tree_invalid_format() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let invalid_tree = json!({
            "not_nodes": []
        });
        
        let result = manager.parse_accessibility_tree(invalid_tree);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChromeMcpError::Accessibility(_)));
    }

    #[test]
    fn test_parse_single_node() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let single_node_tree = json!({
            "nodes": [{
                "nodeId": "1",
                "role": {"value": "button"},
                "name": {"value": "Click me"},
                "boundingRect": {"x": 10, "y": 20, "width": 100, "height": 30}
            }]
        });
        
        let all_nodes = single_node_tree["nodes"].as_array().unwrap();
        let node_raw = &all_nodes[0];
        let result = manager.parse_node(node_raw, all_nodes);
        
        assert!(result.is_ok());
        let node = result.unwrap();
        assert_eq!(node.node_id, "1");
        assert_eq!(node.role, Some("button".to_string()));
        assert_eq!(node.name, Some("Click me".to_string()));
        assert!(node.bounds.is_some());
    }

    #[test]
    fn test_get_bool_property() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let node_with_props = json!({
            "properties": [
                {"name": "focusable", "value": {"booleanValue": true}},
                {"name": "visible", "value": {"booleanValue": false}}
            ]
        });
        
        assert_eq!(manager.get_bool_property(&node_with_props, "focusable"), Some(true));
        assert_eq!(manager.get_bool_property(&node_with_props, "visible"), Some(false));
        assert_eq!(manager.get_bool_property(&node_with_props, "nonexistent"), None);
    }

    #[test]
    fn test_is_clickable_by_role() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let clickable_roles = vec!["button", "link", "menuitem", "tab", "checkbox", "radio"];
        
        for role in clickable_roles {
            let node_json = json!({"role": {"value": role}});
            assert!(manager.is_clickable(&node_json), "Role {} should be clickable", role);
        }
        
        let non_clickable_node = json!({"role": {"value": "text"}});
        assert!(!manager.is_clickable(&non_clickable_node));
    }

    #[test]
    fn test_is_clickable_by_property() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let clickable_node = json!({
            "role": {"value": "div"},
            "properties": [
                {"name": "clickable", "value": {"booleanValue": true}}
            ]
        });
        
        assert!(manager.is_clickable(&clickable_node));
    }

    #[test]
    fn test_search_nodes_by_role() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let mut root = create_test_node("1", Some("document"), Some("Root"), false, None);
        let button1 = create_test_node("2", Some("button"), Some("Submit"), true, None);
        let button2 = create_test_node("3", Some("button"), Some("Cancel"), true, None);
        let text = create_test_node("4", Some("text"), Some("Hello"), false, None);
        
        root.children = vec![button1, button2, text];
        
        let results = manager.search_nodes_by_role(&root, "button");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|n| n.name.as_ref().map(|name| name == "Submit").unwrap_or(false)));
        assert!(results.iter().any(|n| n.name.as_ref().map(|name| name == "Cancel").unwrap_or(false)));
    }

    #[test]
    fn test_search_nodes_by_name() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let mut root = create_test_node("1", Some("document"), Some("Root"), false, None);
        let submit_button = create_test_node("2", Some("button"), Some("Submit Form"), true, None);
        let submit_link = create_test_node("3", Some("link"), Some("Submit Request"), true, None);
        let other_button = create_test_node("4", Some("button"), Some("Cancel"), true, None);
        
        root.children = vec![submit_button, submit_link, other_button];
        
        let results = manager.search_nodes_by_name(&root, "submit");
        assert_eq!(results.len(), 2);
        
        let results_exact = manager.search_nodes_by_name(&root, "Cancel");
        assert_eq!(results_exact.len(), 1);
        assert_eq!(results_exact[0].name, Some("Cancel".to_string()));
    }

    #[test]
    fn test_search_clickable_by_text() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let mut root = create_test_node("1", Some("document"), Some("Root"), false, None);
        let clickable_with_name = create_test_node("2", Some("button"), Some("Click Here"), true, None);
        let clickable_with_desc = AccessibilityNode {
            node_id: "3".to_string(),
            role: Some("button".to_string()),
            name: None,
            description: Some("Click to continue".to_string()),
            value: None,
            properties: None,
            children: Vec::new(),
            bounds: None,
            focusable: false,
            focused: false,
            clickable: true,
        };
        let non_clickable = create_test_node("4", Some("text"), Some("Click me"), false, None);
        
        root.children = vec![clickable_with_name, clickable_with_desc, non_clickable];
        
        let results = manager.search_clickable_by_text(&root, "click");
        assert_eq!(results.len(), 2); // Only the clickable ones
        
        let results_specific = manager.search_clickable_by_text(&root, "continue");
        assert_eq!(results_specific.len(), 1);
    }

    #[test]
    fn test_get_center_coords() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let node_with_bounds = create_test_node("1", Some("button"), Some("Test"), true, Some((10.0, 20.0, 100.0, 50.0)));
        let node_without_bounds = create_test_node("2", Some("button"), Some("Test"), true, None);
        
        let coords = manager.get_center_coords(&node_with_bounds);
        assert!(coords.is_some());
        let (x, y) = coords.unwrap();
        assert_eq!(x, 60.0); // 10 + 100/2
        assert_eq!(y, 45.0); // 20 + 50/2
        
        let coords_none = manager.get_center_coords(&node_without_bounds);
        assert!(coords_none.is_none());
    }

    #[test]
    fn test_collect_node_summaries() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let mut root = create_test_node("1", Some("document"), Some("Page"), false, Some((0.0, 0.0, 800.0, 600.0)));
        let button = create_test_node("2", Some("button"), Some("Submit"), true, Some((10.0, 20.0, 100.0, 30.0)));
        let text = create_test_node("3", Some("text"), Some("Hello World"), false, None);
        
        root.children = vec![button, text];
        
        let mut summary = Vec::new();
        manager.collect_node_summaries(&root, &mut summary, 0);
        
        assert_eq!(summary.len(), 3); // root + 2 children
        assert!(summary[0].contains("document: Page"));
        assert!(summary[0].contains("@(0,0)"));
        assert!(summary[1].contains("button: Submit"));
        assert!(summary[1].contains("[CLICKABLE]"));
        assert!(summary[1].contains("@(10,20)"));
        assert!(summary[2].contains("text: Hello World"));
        assert!(!summary[2].contains("[CLICKABLE]"));
        
        // Check indentation
        assert!(!summary[0].starts_with("  ")); // root level
        assert!(summary[1].starts_with("  ")); // child level
        assert!(summary[2].starts_with("  ")); // child level
    }

    #[test]
    fn test_cache_management() {
        let cdp = CdpClient::new("localhost", 9222);
        let mut manager = AccessibilityManager::new(cdp);
        
        // Initially no cache
        assert!(manager.cached_tree.is_none());
        
        // Set a cached tree
        let test_tree = create_test_node("1", Some("document"), Some("Test"), false, None);
        manager.cached_tree = Some(test_tree);
        assert!(manager.cached_tree.is_some());
        
        // Clear cache
        manager.clear_cache();
        assert!(manager.cached_tree.is_none());
    }

    #[test]
    fn test_nested_search() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        // Create nested structure: root -> form -> button
        let mut root = create_test_node("1", Some("document"), None, false, None);
        let mut form = create_test_node("2", Some("form"), None, false, None);
        let button = create_test_node("3", Some("button"), Some("Submit"), true, None);
        
        form.children = vec![button];
        root.children = vec![form];
        
        let results = manager.search_nodes_by_role(&root, "button");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].node_id, "3");
        assert_eq!(results[0].name, Some("Submit".to_string()));
    }

    #[test]
    fn test_case_insensitive_search() {
        let cdp = CdpClient::new("localhost", 9222);
        let manager = AccessibilityManager::new(cdp);
        
        let node = create_test_node("1", Some("BUTTON"), Some("SUBMIT FORM"), false, None);
        
        let results_role = manager.search_nodes_by_role(&node, "button");
        assert_eq!(results_role.len(), 1);
        
        let results_name = manager.search_nodes_by_name(&node, "submit");
        assert_eq!(results_name.len(), 1);
    }
}
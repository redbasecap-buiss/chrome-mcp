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
//! Per-tool confidence thresholds — adjustable confidence per tool name.

use std::collections::HashMap;

/// Per-tool confidence thresholds. Falls back to a global default
/// when no tool-specific threshold has been set.
#[derive(Debug, Clone)]
pub struct ToolConfidenceMap {
    default_threshold: f32,
    per_tool: HashMap<String, f32>,
}

impl ToolConfidenceMap {
    pub fn new(default_threshold: f32) -> Self {
        Self {
            default_threshold,
            per_tool: HashMap::new(),
        }
    }

    /// Get the confidence threshold for a specific tool.
    pub fn get_threshold(&self, tool_name: &str) -> f32 {
        self.per_tool
            .get(tool_name)
            .copied()
            .unwrap_or(self.default_threshold)
    }

    /// Set a tool-specific threshold.
    pub fn set_threshold(&mut self, tool_name: &str, threshold: f32) {
        self.per_tool
            .insert(tool_name.to_string(), threshold.clamp(0.0, 1.0));
    }

    /// Get the global default threshold.
    pub fn default_threshold(&self) -> f32 {
        self.default_threshold
    }

    /// Number of tool-specific overrides.
    pub fn overrides_count(&self) -> usize {
        self.per_tool.len()
    }

    /// All tool names with custom thresholds.
    pub fn custom_tools(&self) -> Vec<&str> {
        self.per_tool.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_threshold_for_unknown_tool() {
        let map = ToolConfidenceMap::new(0.7);
        assert_eq!(map.get_threshold("unknown_tool"), 0.7);
    }

    #[test]
    fn test_custom_threshold_overrides_default() {
        let mut map = ToolConfidenceMap::new(0.7);
        map.set_threshold("shell", 0.9);
        assert_eq!(map.get_threshold("shell"), 0.9);
        assert_eq!(map.get_threshold("web_search"), 0.7); // still default
    }

    #[test]
    fn test_threshold_clamped_to_range() {
        let mut map = ToolConfidenceMap::new(0.7);
        map.set_threshold("risky_tool", 1.5);
        assert_eq!(map.get_threshold("risky_tool"), 1.0);
        map.set_threshold("easy_tool", -0.5);
        assert_eq!(map.get_threshold("easy_tool"), 0.0);
    }

    #[test]
    fn test_overrides_count() {
        let mut map = ToolConfidenceMap::new(0.7);
        assert_eq!(map.overrides_count(), 0);
        map.set_threshold("shell", 0.9);
        map.set_threshold("web", 0.5);
        assert_eq!(map.overrides_count(), 2);
    }
}

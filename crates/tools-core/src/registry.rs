//! Tool registry for dynamic tool management.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

use crate::metadata::{ToolCategory, ToolMetadata};
use crate::{DynTool, RoutingContext, Tool, ToolPermissions};
use common::{Result, ToolError};

/// Registry for agent tools
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
    metadata: HashMap<String, ToolMetadata>,
    usage_counts: HashMap<String, u64>,
    cached_definitions: Mutex<Option<Arc<Vec<Value>>>>,
    permissions: Option<ToolPermissions>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            metadata: HashMap::new(),
            usage_counts: HashMap::new(),
            cached_definitions: Mutex::new(None),
            permissions: None,
        }
    }

    /// Set the permission configuration for this registry.
    pub fn set_permissions(&mut self, permissions: ToolPermissions) {
        self.permissions = Some(permissions);
    }

    /// Register a tool
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        let meta = tool.metadata();
        debug!("Registering tool: {}", name);
        self.metadata.insert(name.clone(), meta);
        self.tools.insert(name, Arc::new(tool));
        self.invalidate_cache();
    }

    /// Register a pre-wrapped dynamic tool (used by FeaturePackage).
    pub fn register_dyn(&mut self, tool: DynTool) {
        let name = tool.name().to_string();
        let meta = tool.metadata();
        debug!("Registering dynamic tool: {}", name);
        self.metadata.insert(name.clone(), meta);
        self.tools.insert(name, tool);
        self.invalidate_cache();
    }

    /// Unregister a tool by name
    pub fn unregister(&mut self, name: &str) {
        self.tools.remove(name);
        self.metadata.remove(name);
        self.usage_counts.remove(name);
        self.invalidate_cache();
    }

    /// Unregister all tools whose name starts with the given prefix.
    ///
    /// Returns the number of tools removed. Used to cleanly remove all
    /// tools from an MCP server (e.g., prefix `"mcp_linear_"`).
    pub fn unregister_by_prefix(&mut self, prefix: &str) -> usize {
        let before = self.tools.len();
        self.tools.retain(|name, _| !name.starts_with(prefix));
        self.metadata.retain(|name, _| !name.starts_with(prefix));
        self.usage_counts
            .retain(|name, _| !name.starts_with(prefix));
        let removed = before - self.tools.len();
        if removed > 0 {
            debug!("Unregistered {} tools with prefix '{}'", removed, prefix);
            self.invalidate_cache();
        }
        removed
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<DynTool> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool is registered
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all tool definitions in OpenAI function-calling format.
    /// Uses interior mutability so only a shared reference is needed.
    /// Returns `Arc<Vec<Value>>` so cache hits are an atomic increment, not a deep clone.
    pub fn get_definitions(&self) -> Arc<Vec<Value>> {
        let mut cache = self.cached_definitions.lock().expect("cache lock poisoned");
        if let Some(defs) = cache.as_ref() {
            return Arc::clone(defs);
        }

        // First time: build and cache all tool schemas
        let definitions: Vec<Value> = self.tools.values().map(|tool| tool.to_schema()).collect();
        debug!("Cached {} tool definitions", definitions.len());
        let arc = Arc::new(definitions);
        *cache = Some(Arc::clone(&arc));
        arc
    }

    /// Execute a tool by name with given parameters and routing context
    pub async fn execute(&self, name: &str, params: Value, ctx: &RoutingContext) -> Result<String> {
        let tool = self.prepare(name, &params, ctx)?;

        match tool.execute(params, ctx).await {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!("Tool {} execution failed: {}", name, e);
                Err(e)
            }
        }
    }

    /// Look up a tool, check permissions, and validate params — without executing.
    ///
    /// Returns the tool as a cloned `DynTool` (Arc) so the caller can drop the
    /// registry borrow before calling `tool.execute()`. This prevents deadlocks
    /// when a tool (e.g., `delegate`) needs write access to the same registry
    /// during execution.
    pub fn prepare(&self, name: &str, params: &Value, ctx: &RoutingContext) -> Result<DynTool> {
        let tool = self
            .tools
            .get(name)
            .cloned()
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        // Check permissions if configured
        if let Some(ref perms) = self.permissions {
            let required = tool.permission_level();
            let channel = ctx.channel.as_str();
            if !perms.is_allowed(channel, required) {
                return Err(ToolError::PermissionDenied(format!(
                    "Tool '{}' requires {} permission, channel '{}' has insufficient access",
                    name, required, channel
                ))
                .into());
            }
        }

        // Validate parameters
        let errors = tool.validate_params(params);
        if !errors.is_empty() {
            return Err(ToolError::InvalidParams(errors.join("; ")).into());
        }

        Ok(tool)
    }

    /// Get metadata for a tool by name.
    pub fn get_metadata(&self, name: &str) -> Option<&ToolMetadata> {
        self.metadata.get(name)
    }

    /// Get tool names that belong to a given category.
    pub fn by_category(&self, category: &ToolCategory) -> Vec<&str> {
        self.metadata
            .iter()
            .filter(|(_, meta)| &meta.category == category)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Record a tool usage for tracking.
    pub fn record_usage(&mut self, name: &str) {
        *self.usage_counts.entry(name.to_string()).or_insert(0) += 1;
    }

    /// Get the top N most-used tools, sorted by usage count descending.
    pub fn top_used(&self, n: usize) -> Vec<(&str, u64)> {
        let mut counts: Vec<_> = self
            .usage_counts
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1));
        counts.truncate(n);
        counts
    }

    /// Simple keyword search across tool names, descriptions, and tags.
    pub fn search_tools(&self, query: &str, limit: usize) -> Vec<(String, f64)> {
        let query_lower = query.to_lowercase();
        let terms: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scores: Vec<(String, f64)> = self
            .tools
            .iter()
            .map(|(name, tool)| {
                let desc = tool.description().to_lowercase();
                let name_lower = name.to_lowercase();
                let meta = self.metadata.get(name);

                let mut score = 0.0;
                for term in &terms {
                    if name_lower.contains(term) {
                        score += 3.0;
                    }
                    if desc.contains(term) {
                        score += 1.0;
                    }
                    if let Some(m) = meta {
                        if m.tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(term))
                        {
                            score += 2.0;
                        }
                    }
                }
                (name.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scores.sort_by(|a, b| b.1.total_cmp(&a.1));
        scores.truncate(limit);
        scores
    }

    /// Get list of registered tool names
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get the number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    fn invalidate_cache(&self) {
        *self.cached_definitions.lock().expect("cache lock poisoned") = None;
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct FakeSearchTool;

    #[async_trait]
    impl Tool for FakeSearchTool {
        fn name(&self) -> &str {
            "search"
        }
        fn description(&self) -> &str {
            "Search for files and content"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object"})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok("ok".into())
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata {
                category: ToolCategory::Search,
                tags: vec!["file".into(), "content".into(), "grep".into()],
                cost_hint: CostHint::Free,
                ..Default::default()
            }
        }
    }

    struct FakeWebTool;

    #[async_trait]
    impl Tool for FakeWebTool {
        fn name(&self) -> &str {
            "web_search"
        }
        fn description(&self) -> &str {
            "Search the web for information"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object"})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok("ok".into())
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata {
                category: ToolCategory::Web,
                tags: vec!["web".into(), "search".into(), "internet".into()],
                cost_hint: CostHint::Low,
                ..Default::default()
            }
        }
    }

    #[test]
    fn test_registry_stores_metadata() {
        let mut reg = ToolRegistry::new();
        reg.register(FakeSearchTool);

        let meta = reg.get_metadata("search");
        assert!(meta.is_some());
        assert_eq!(meta.unwrap().category, ToolCategory::Search);
    }

    #[test]
    fn test_registry_by_category() {
        let mut reg = ToolRegistry::new();
        reg.register(FakeSearchTool);
        reg.register(FakeWebTool);

        let search_tools = reg.by_category(&ToolCategory::Search);
        assert_eq!(search_tools.len(), 1);
        assert_eq!(search_tools[0], "search");
    }

    #[test]
    fn test_registry_usage_tracking() {
        let mut reg = ToolRegistry::new();
        reg.register(FakeSearchTool);

        reg.record_usage("search");
        reg.record_usage("search");
        reg.record_usage("search");

        let top = reg.top_used(5);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].0, "search");
        assert_eq!(top[0].1, 3);
    }

    #[test]
    fn test_registry_search_by_query() {
        let mut reg = ToolRegistry::new();
        reg.register(FakeSearchTool);
        reg.register(FakeWebTool);

        let results = reg.search_tools("web internet", 10);
        assert!(!results.is_empty());
        // web_search should rank higher for "web internet"
        assert_eq!(results[0].0, "web_search");
    }
}

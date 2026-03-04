//! Tool registry for dynamic tool management.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

use crate::{DynTool, RoutingContext, Tool, ToolPermissions};
use common::{Result, ToolError};

/// Registry for agent tools
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
    cached_definitions: Mutex<Option<Arc<Vec<Value>>>>,
    permissions: Option<ToolPermissions>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
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
        debug!("Registering tool: {}", name);
        self.tools.insert(name, Arc::new(tool));
        self.invalidate_cache();
    }

    /// Register a pre-wrapped dynamic tool (used by FeaturePackage).
    pub fn register_dyn(&mut self, tool: DynTool) {
        let name = tool.name().to_string();
        debug!("Registering dynamic tool: {}", name);
        self.tools.insert(name, tool);
        self.invalidate_cache();
    }

    /// Unregister a tool by name
    pub fn unregister(&mut self, name: &str) {
        self.tools.remove(name);
        self.invalidate_cache();
    }

    /// Unregister all tools whose name starts with the given prefix.
    ///
    /// Returns the number of tools removed. Used to cleanly remove all
    /// tools from an MCP server (e.g., prefix `"mcp_linear_"`).
    pub fn unregister_by_prefix(&mut self, prefix: &str) -> usize {
        let before = self.tools.len();
        self.tools.retain(|name, _| !name.starts_with(prefix));
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
        let tool = self
            .tools
            .get(name)
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
        let errors = tool.validate_params(&params);
        if !errors.is_empty() {
            return Err(ToolError::InvalidParams(errors.join("; ")).into());
        }

        // Execute the tool
        match tool.execute(params, ctx).await {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!("Tool {} execution failed: {}", name, e);
                Err(e)
            }
        }
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

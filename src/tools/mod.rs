//! Tool system for agent capabilities.

pub mod cron_tool;
pub mod filesystem;
pub mod message;
pub mod registry;
pub mod shell;
pub mod spawn;
pub mod web;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, RwLock};

use crate::error::Result;

/// Shared context for tools that need routing information.
/// This allows tools like message, spawn, and cron to share a single
/// context instance instead of each maintaining their own.
#[derive(Clone)]
pub struct ToolContext {
    inner: Arc<RwLock<(String, String)>>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolContext {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new((String::new(), String::new()))),
        }
    }

    pub fn set(&self, channel: &str, chat_id: &str) {
        let mut ctx = self.inner.write().unwrap();
        ctx.0 = channel.to_string();
        ctx.1 = chat_id.to_string();
    }

    pub fn get(&self) -> (String, String) {
        self.inner.read().unwrap().clone()
    }
}

/// Trait for agent tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name for function calls (e.g., "read_file")
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// JSON Schema for parameters
    fn parameters(&self) -> Value;

    /// Execute the tool with given arguments
    async fn execute(&self, args: Value) -> Result<String>;

    /// Convert to OpenAI function schema format
    fn to_schema(&self) -> Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": self.parameters()
            }
        })
    }

    /// Validate parameters against JSON Schema
    fn validate_params(&self, params: &Value) -> Vec<String> {
        let schema = self.parameters();
        validate_value(params, &schema, "")
    }

    /// Set the current conversation context for tools that need routing info.
    /// Default implementation is a no-op (most tools don't need context).
    fn set_context(&self, _channel: &str, _chat_id: &str) {
        // No-op by default
    }
}

/// Type alias for dynamic tool
pub type DynTool = Arc<dyn Tool>;

/// Validate a value against a JSON Schema
fn validate_value(val: &Value, schema: &Value, path: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let label = if path.is_empty() {
        "parameter".to_string()
    } else {
        path.to_string()
    };

    let schema_type = schema.get("type").and_then(|t| t.as_str());

    // Type validation
    match schema_type {
        Some("string") => {
            if !val.is_string() {
                errors.push(format!("{} should be string", label));
                return errors;
            }
            let s = val.as_str().unwrap();
            if let Some(min_len) = schema.get("minLength").and_then(|v| v.as_u64()) {
                if s.len() < min_len as usize {
                    errors.push(format!("{} must be at least {} chars", label, min_len));
                }
            }
            if let Some(max_len) = schema.get("maxLength").and_then(|v| v.as_u64()) {
                if s.len() > max_len as usize {
                    errors.push(format!("{} must be at most {} chars", label, max_len));
                }
            }
        }
        Some("integer") => {
            if !val.is_i64() && !val.is_u64() {
                errors.push(format!("{} should be integer", label));
                return errors;
            }
            let n = val.as_i64().unwrap_or(0);
            if let Some(min) = schema.get("minimum").and_then(|v| v.as_i64()) {
                if n < min {
                    errors.push(format!("{} must be >= {}", label, min));
                }
            }
            if let Some(max) = schema.get("maximum").and_then(|v| v.as_i64()) {
                if n > max {
                    errors.push(format!("{} must be <= {}", label, max));
                }
            }
        }
        Some("number") => {
            if !val.is_f64() && !val.is_i64() && !val.is_u64() {
                errors.push(format!("{} should be number", label));
                return errors;
            }
        }
        Some("boolean") => {
            if !val.is_boolean() {
                errors.push(format!("{} should be boolean", label));
                return errors;
            }
        }
        Some("array") => {
            if !val.is_array() {
                errors.push(format!("{} should be array", label));
                return errors;
            }
            if let Some(items_schema) = schema.get("items") {
                if let Some(arr) = val.as_array() {
                    for (i, item) in arr.iter().enumerate() {
                        let item_path = format!("{}[{}]", path, i);
                        errors.extend(validate_value(item, items_schema, &item_path));
                    }
                }
            }
        }
        Some("object") => {
            if !val.is_object() {
                errors.push(format!("{} should be object", label));
                return errors;
            }
            let obj = val.as_object().unwrap();

            // Check required fields
            if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                for req in required {
                    if let Some(field_name) = req.as_str() {
                        if !obj.contains_key(field_name) {
                            let field_path = if path.is_empty() {
                                field_name.to_string()
                            } else {
                                format!("{}.{}", path, field_name)
                            };
                            errors.push(format!("missing required {}", field_path));
                        }
                    }
                }
            }

            // Validate properties
            if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                for (key, value) in obj.iter() {
                    if let Some(prop_schema) = properties.get(key) {
                        let prop_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{}.{}", path, key)
                        };
                        errors.extend(validate_value(value, prop_schema, &prop_path));
                    }
                }
            }
        }
        _ => {}
    }

    // Enum validation
    if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
        if !enum_values.contains(val) {
            errors.push(format!("{} must be one of {:?}", label, enum_values));
        }
    }

    errors
}

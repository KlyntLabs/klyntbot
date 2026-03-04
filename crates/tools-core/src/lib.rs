//! Core tool framework for klyntbot feature packages.
//!
//! Provides the `Tool` trait, `FeaturePackage` trait, `ToolRegistry`,
//! `ParamExtractor`, and derive macros that eliminate boilerplate
//! when building feature packages.

pub mod config_persistence;
pub mod feature;
pub mod pagination;
pub mod params;
pub mod permissions;
pub mod registry;
pub mod search;

// Re-export proc macros
pub use tools_core_macros::{tool_actions, ActionParams, DomainEnum, Tool, ToolParams};

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use common::{ChannelName, ChatId, FormResponse, InteractionRequest, Result};

// Re-export submodule types
pub use config_persistence::ConfigPersistence;
pub use feature::{FeatureMigration, FeaturePackage, HealthStatus};
pub use pagination::Page;
pub use params::ParamExtractor;
pub use permissions::{PermissionLevel, ToolPermissions};
pub use registry::ToolRegistry;
pub use search::{rrf_merge, Searchable};

/// Bundle sent from ask_user tool to the CLI.
/// Each request carries its own response channel.
pub struct InteractionBundle {
    pub request: InteractionRequest,
    pub response_tx: oneshot::Sender<FormResponse>,
}

/// Trait for cascading progress updates (KR → Objective).
///
/// Defined in `tools-core` (Layer 1) so both `tools` (OkrTool) and `feature-todo`
/// (TaskTool) can consume it without circular deps. Implemented in `agent` (Layer 5).
#[async_trait]
pub trait ProgressHandler: Send + Sync {
    /// Recalculate progress for a key result and cascade to its parent objective.
    async fn recalculate_kr_progress(&self, key_result_id: &str) -> Result<()>;
}

/// Trait for channels that support structured interactions (buttons, menus, etc.).
///
/// Defined in `tools-core` (Layer 3) to avoid circular deps with `channels` (Layer 4).
/// Channel implementations that support native UI elements (Telegram inline keyboards,
/// Discord buttons/selects, Slack Block Kit) implement this trait.
#[async_trait]
pub trait InteractionChannel: Send + Sync {
    /// Whether this channel supports structured interactions.
    fn supports_interaction(&self) -> bool;

    /// Send a structured interaction request and wait for the user's response.
    ///
    /// The channel renders questions using platform-native UI (buttons, menus)
    /// and returns the user's selections.
    async fn send_interaction(
        &self,
        chat_id: &str,
        request: &InteractionRequest,
    ) -> Result<FormResponse>;
}

/// Routing context for tools that need channel/chat information.
/// This is passed explicitly to execute() instead of using shared mutable state.
///
/// The optional `interaction_tx` allows the ask_user tool to send interaction
/// requests to the CLI during execution. Each request includes a oneshot channel
/// for the response, enabling blocking behavior within the tool.
#[derive(Clone)]
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    /// Channel for ask_user tool to send interaction bundles.
    /// Only available when running in CLI chat mode (TTY).
    /// Clone-safe: mpsc::Sender is Clone.
    pub interaction_tx: Option<mpsc::Sender<InteractionBundle>>,
    /// When true, the user receives responses directly via the event stream
    /// (CLI or dashboard WebSocket), so the `message` tool should return
    /// content inline rather than publishing to the bus.
    pub is_direct_mode: bool,
    /// Channel for tools to emit entity cards (e.g., after creating a task).
    /// The execution layer drains this and emits AgentEvent::EntityCreated.
    pub entity_tx: Option<mpsc::Sender<common::EntityCard>>,
    /// Platform-native interaction channel (Telegram buttons, Discord selects, etc.).
    /// When present, `ask_user` uses this for structured UI instead of text fallback.
    pub interaction_channel: Option<Arc<dyn InteractionChannel>>,
}

impl RoutingContext {
    /// Non-interactive mode (bus-driven channels, non-TTY).
    pub fn new(channel: ChannelName, chat_id: ChatId) -> Self {
        Self {
            channel,
            chat_id,
            interaction_tx: None,
            is_direct_mode: false,
            entity_tx: None,
            interaction_channel: None,
        }
    }

    /// Interactive direct mode (CLI/dashboard) with user interaction support.
    pub fn with_interaction(
        channel: ChannelName,
        chat_id: ChatId,
        interaction_tx: mpsc::Sender<InteractionBundle>,
    ) -> Self {
        Self {
            channel,
            chat_id,
            interaction_tx: Some(interaction_tx),
            is_direct_mode: true,
            entity_tx: None,
            interaction_channel: None,
        }
    }
}

/// Typed tool parameters — generates JSON schema and parses from `serde_json::Value`.
///
/// Derive with `#[derive(ToolParams)]`. Fields use `#[param(required)]` for required
/// params, and doc comments become schema descriptions.
pub trait ToolParams: Sized {
    /// Generate JSON Schema for these parameters.
    fn json_schema() -> Value;

    /// Parse parameters from a JSON value.
    fn from_args(args: Value) -> common::Result<Self>;
}

/// Typed tool execution — implement this for your tool's business logic,
/// then use `#[derive(Tool)]` to generate the untyped `Tool` trait bridge.
#[async_trait]
pub trait ToolExecute: Send + Sync {
    /// The typed parameter struct (must implement `ToolParams`).
    type Params: ToolParams;

    /// Execute the tool with typed parameters.
    async fn execute(&self, params: Self::Params, ctx: &RoutingContext) -> common::Result<String>;
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

    /// Execute the tool with given arguments and routing context
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;

    /// Permission level required to use this tool.
    /// Defaults to `Standard`. Override for sensitive tools.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Standard
    }

    /// Optional per-tool timeout override.
    ///
    /// When `Some(duration)`, the execution core uses this instead of the
    /// default tool timeout. Used by MCP tools to respect per-server
    /// `tool_timeout_sec` configuration.
    fn custom_timeout(&self) -> Option<std::time::Duration> {
        None
    }

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
            if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if !re.is_match(s) {
                        errors.push(format!("{} must match pattern {}", label, pattern));
                    }
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
            let n = val.as_f64().unwrap_or(0.0);
            if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
                if n < min {
                    errors.push(format!("{} must be >= {}", label, min));
                }
            }
            if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
                if n > max {
                    errors.push(format!("{} must be <= {}", label, max));
                }
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
            if let Some(arr) = val.as_array() {
                if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
                    if arr.len() < min as usize {
                        errors.push(format!("{} must have at least {} items", label, min));
                    }
                }
                if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
                    if arr.len() > max as usize {
                        errors.push(format!("{} must have at most {} items", label, max));
                    }
                }
                if let Some(items_schema) = schema.get("items") {
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

            // Check additionalProperties: false
            if let Some(false) = schema.get("additionalProperties").and_then(|v| v.as_bool()) {
                if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                    for key in obj.keys() {
                        if !properties.contains_key(key) {
                            let prop_path = if path.is_empty() {
                                key.clone()
                            } else {
                                format!("{}.{}", path, key)
                            };
                            errors.push(format!("unexpected property {}", prop_path));
                        }
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

    // oneOf: must match exactly one subschema
    if let Some(schemas) = schema.get("oneOf").and_then(|s| s.as_array()) {
        let match_count = schemas
            .iter()
            .filter(|s| validate_value(val, s, path).is_empty())
            .count();
        if match_count != 1 {
            errors.push(format!(
                "{} must match exactly one of oneOf schemas (matched {})",
                label, match_count
            ));
        }
    }

    // anyOf: must match at least one subschema
    if let Some(schemas) = schema.get("anyOf").and_then(|s| s.as_array()) {
        let matches_any = schemas
            .iter()
            .any(|s| validate_value(val, s, path).is_empty());
        if !matches_any {
            errors.push(format!(
                "{} must match at least one of anyOf schemas",
                label
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_one_of_matches_exactly_one() {
        let schema = json!({
            "oneOf": [
                { "type": "string" },
                { "type": "integer" }
            ]
        });
        // String matches exactly one
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        // Integer matches exactly one
        assert!(validate_value(&json!(42), &schema, "").is_empty());
        // Boolean matches none → error
        assert!(!validate_value(&json!(true), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_any_of_matches_at_least_one() {
        let schema = json!({
            "anyOf": [
                { "type": "string" },
                { "type": "integer" }
            ]
        });
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        assert!(validate_value(&json!(42), &schema, "").is_empty());
        assert!(!validate_value(&json!(true), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_pattern() {
        let schema = json!({
            "type": "string",
            "pattern": "^[a-z]+$"
        });
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        assert!(!validate_value(&json!("Hello123"), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_additional_properties_false() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": false
        });
        // Only declared property — OK
        assert!(validate_value(&json!({"name": "test"}), &schema, "").is_empty());
        // Extra property — error
        assert!(!validate_value(&json!({"name": "test", "extra": true}), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_min_max_items() {
        let schema = json!({
            "type": "array",
            "items": { "type": "integer" },
            "minItems": 2,
            "maxItems": 4
        });
        assert!(!validate_value(&json!([1]), &schema, "").is_empty()); // too few
        assert!(validate_value(&json!([1, 2]), &schema, "").is_empty()); // ok
        assert!(validate_value(&json!([1, 2, 3, 4]), &schema, "").is_empty()); // ok
        assert!(!validate_value(&json!([1, 2, 3, 4, 5]), &schema, "").is_empty());
        // too many
    }

    #[test]
    fn test_validate_number_minimum_maximum() {
        let schema = json!({
            "type": "number",
            "minimum": 0.0,
            "maximum": 1.0
        });
        assert!(validate_value(&json!(0.5), &schema, "").is_empty());
        assert!(!validate_value(&json!(-0.1), &schema, "").is_empty());
        assert!(!validate_value(&json!(1.5), &schema, "").is_empty());
    }
}

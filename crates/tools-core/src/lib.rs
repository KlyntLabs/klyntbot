//! Core tool framework for klyntbot feature packages.
//!
//! Provides the `Tool` trait, `FeaturePackage` trait, `ToolRegistry`,
//! `ParamExtractor`, and derive macros that eliminate boilerplate
//! when building feature packages.

// ── Modules ─────────────────────────────────────────────────────────────

pub mod config_persistence;
pub mod feature;
pub mod interceptor;
pub mod metadata;
pub mod pagination;
pub mod params;
pub mod permissions;
pub mod registry;
pub mod routing;
pub mod search;
mod validation;

// ── Re-exports: proc macros ─────────────────────────────────────────────

pub use tools_core_macros::{tool_actions, ActionParams, DomainEnum, Tool, ToolParams};

// ── Re-exports: submodule types ─────────────────────────────────────────

pub use config_persistence::ConfigPersistence;
pub use feature::{FeatureMigration, FeaturePackage, HealthStatus};
pub use interceptor::{InterceptorChain, ToolCallInterceptor};
pub use metadata::{CostHint, ToolCategory, ToolMetadata, ToolSource};
pub use pagination::Page;
pub use params::ParamExtractor;
pub use permissions::{PermissionLevel, ToolPermissions};
pub use registry::ToolRegistry;
pub use routing::{InteractionBundle, InteractionChannel, ProgressHandler, RoutingContext};
pub use search::{rrf_merge, rrf_merge_triple, Searchable};

// ── Core traits ─────────────────────────────────────────────────────────

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use common::Result;

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

/// Trait for agent tools.
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

    /// Rich metadata for discovery. Override to provide category, tags, etc.
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }

    /// Whether this tool can be safely dispatched in parallel with other
    /// `is_concurrency_safe == true` tools in the same iteration.
    ///
    /// Returns `false` by default. Override to `true` for read-only tools
    /// (e.g., `read`, `glob`, `grep`, `recall_*`) that have no observable
    /// side effects on the filesystem, network, or shared mutable state.
    ///
    /// The execution loop partitions tool calls by this flag: safe tools
    /// run via `futures::future::join_all`; unsafe tools run sequentially.
    fn is_concurrency_safe(&self, _args: &Value) -> bool {
        false
    }

    /// Channels in which this tool is visible to the LLM. Default = ALL.
    /// Override to restrict — tools that need approval UI return CODING_ONLY.
    fn allowed_channels(&self) -> common::ChannelMask {
        common::ChannelMask::ALL
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
        validation::validate_value(params, &schema, "")
    }
}

/// Type alias for dynamic tool.
pub type DynTool = Arc<dyn Tool>;

/// Structured tool output — backward compatible with plain String.
///
/// Tools return `Result<String>` today. `ToolOutput` provides an opt-in
/// upgrade path for tools that want to return structured data alongside
/// a human-readable summary. The `From<String>` impl ensures backward
/// compatibility — existing `String` results convert automatically.
#[derive(Debug, Clone)]
pub enum ToolOutput {
    /// Plain text (current behavior).
    Text(String),
    /// Structured: summary for LLM context, data for UI/MCP.
    Structured {
        summary: String,
        data: serde_json::Value,
    },
}

impl From<String> for ToolOutput {
    fn from(s: String) -> Self {
        ToolOutput::Text(s)
    }
}

impl From<&str> for ToolOutput {
    fn from(s: &str) -> Self {
        ToolOutput::Text(s.to_string())
    }
}

impl ToolOutput {
    /// Get the text representation (summary for Structured, raw text for Text).
    pub fn as_text(&self) -> &str {
        match self {
            ToolOutput::Text(s) => s,
            ToolOutput::Structured { summary, .. } => summary,
        }
    }

    /// Consume into the text representation.
    pub fn into_string(self) -> String {
        match self {
            ToolOutput::Text(s) => s,
            ToolOutput::Structured { summary, .. } => summary,
        }
    }

    /// Try to parse a tool result string into a `ToolOutput`.
    /// Detects the `__STRUCTURED__` prefix convention.
    pub fn parse(result: &str) -> Self {
        if let Some(stripped) = result.strip_prefix("__STRUCTURED__") {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(stripped) {
                if let (Some(summary), Some(data)) = (value["summary"].as_str(), value.get("data"))
                {
                    return ToolOutput::Structured {
                        summary: summary.to_string(),
                        data: data.clone(),
                    };
                }
            }
        }
        ToolOutput::Text(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Minimal Tool fixture for trait-default testing.
    struct DefaultsTool;

    #[async_trait]
    impl Tool for DefaultsTool {
        fn name(&self) -> &str {
            "defaults"
        }
        fn description(&self) -> &str {
            "fixture"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object"})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok(String::new())
        }
    }

    #[test]
    fn is_concurrency_safe_defaults_to_false() {
        let t = DefaultsTool;
        assert!(!t.is_concurrency_safe(&json!({})));
    }

    /// Tool that explicitly opts into concurrency-safe.
    struct ReadOnlyTool;

    #[async_trait]
    impl Tool for ReadOnlyTool {
        fn name(&self) -> &str {
            "readonly"
        }
        fn description(&self) -> &str {
            "fixture"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object"})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok(String::new())
        }
        fn is_concurrency_safe(&self, _args: &Value) -> bool {
            true
        }
    }

    #[test]
    fn is_concurrency_safe_can_be_overridden_to_true() {
        let t = ReadOnlyTool;
        assert!(t.is_concurrency_safe(&json!({})));
    }
}

//! Core tool framework for klyntbot feature packages.
//!
//! Provides the `Tool` trait, `FeaturePackage` trait, `ToolRegistry`,
//! `ParamExtractor`, and derive macros that eliminate boilerplate
//! when building feature packages.

// ── Modules ─────────────────────────────────────────────────────────────

pub mod config_persistence;
pub mod feature;
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
pub use metadata::{CostHint, ToolCategory, ToolExample, ToolMetadata, ToolSource};
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

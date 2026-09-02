//! Tests for `#[derive(Tool)]` macro — generates Tool trait from ToolExecute + metadata.

use async_trait::async_trait;
use common::{ChannelMask, ChannelName, ChatId};
use serde_json::json;
use tools_core::{
    ExposurePolicy, McpExposure, RoutingContext, Tool as ToolTrait, ToolCategory, ToolExecute,
    ToolParams,
};

// ── Params ──────────────────────────────────────────────────

#[derive(Debug, ToolParams)]
pub struct EchoParams {
    /// The text to echo back
    #[param(required)]
    pub text: String,

    /// Number of times to repeat
    pub repeat: Option<u32>,
}

// ── Tool struct ─────────────────────────────────────────────

#[derive(tools_core::Tool)]
#[tool(name = "echo", description = "Echoes text back", params = "EchoParams")]
pub struct EchoTool {
    pub prefix: String,
}

#[async_trait]
impl ToolExecute for EchoTool {
    type Params = EchoParams;
    type Ctx<'a> = ();

    async fn execute<'c>(&self, params: EchoParams, _ctx: ()) -> common::Result<String> {
        let repeat = params.repeat.unwrap_or(1);
        let output: Vec<_> = (0..repeat)
            .map(|_| format!("{}{}", self.prefix, params.text))
            .collect();
        Ok(output.join(" "))
    }
}

// ── Unit params tool ────────────────────────────────────────

#[derive(Debug, ToolParams)]
pub struct NoParams;

#[derive(tools_core::Tool)]
#[tool(name = "noop", description = "Does nothing", params = "NoParams")]
pub struct NoopTool;

#[async_trait]
impl ToolExecute for NoopTool {
    type Params = NoParams;
    type Ctx<'a> = ();

    async fn execute<'c>(&self, _params: NoParams, _ctx: ()) -> common::Result<String> {
        Ok("done".to_string())
    }
}

// ── Tool with metadata ─────────────────────────────────────

#[derive(tools_core::Tool)]
#[tool(
    name = "search_files",
    description = "Search for files by pattern",
    params = "NoParams",
    category = "Search",
    tags = "file,search,pattern",
    cost = "Free"
)]
pub struct SearchFilesTool;

#[async_trait]
impl ToolExecute for SearchFilesTool {
    type Params = NoParams;
    type Ctx<'a> = ();

    async fn execute<'c>(&self, _params: NoParams, _ctx: ()) -> common::Result<String> {
        Ok("found".to_string())
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn ctx() -> RoutingContext {
    RoutingContext::new(ChannelName::from("test"), ChatId::from("123"))
}

// ── Tests ───────────────────────────────────────────────────

#[test]
fn test_name_from_attribute() {
    let tool = EchoTool {
        prefix: "".to_string(),
    };
    assert_eq!(tool.name(), "echo");
}

#[test]
fn test_description_from_attribute() {
    let tool = EchoTool {
        prefix: "".to_string(),
    };
    assert_eq!(tool.description(), "Echoes text back");
}

#[test]
fn test_parameters_generated_from_params_type() {
    let tool = EchoTool {
        prefix: "".to_string(),
    };
    let params = tool.parameters();
    assert_eq!(params["properties"]["text"]["type"], "string");
    assert_eq!(params["properties"]["repeat"]["type"], "integer");
    let required = params["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "text"));
}

#[tokio::test]
async fn test_execute_dispatches_to_tool_execute() {
    let tool = EchoTool {
        prefix: "> ".to_string(),
    };
    let result = ToolTrait::execute(&tool, json!({"text": "hello"}), &ctx())
        .await
        .unwrap();
    assert_eq!(result, "> hello");
}

#[tokio::test]
async fn test_execute_with_optional_param() {
    let tool = EchoTool {
        prefix: "".to_string(),
    };
    let result = ToolTrait::execute(&tool, json!({"text": "hi", "repeat": 3}), &ctx())
        .await
        .unwrap();
    assert_eq!(result, "hi hi hi");
}

#[tokio::test]
async fn test_execute_missing_required_param_errors() {
    let tool = EchoTool {
        prefix: "".to_string(),
    };
    let result = ToolTrait::execute(&tool, json!({}), &ctx()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_noop_tool_works() {
    let tool = NoopTool;
    assert_eq!(tool.name(), "noop");
    let result = ToolTrait::execute(&tool, json!({}), &ctx()).await.unwrap();
    assert_eq!(result, "done");
}

#[test]
fn test_to_schema_includes_all_metadata() {
    let tool = EchoTool {
        prefix: "".to_string(),
    };
    let schema = tool.to_schema();
    assert_eq!(schema["function"]["name"], "echo");
    assert_eq!(schema["function"]["description"], "Echoes text back");
    assert!(schema["function"]["parameters"]["properties"]["text"].is_object());
}

#[test]
fn test_metadata_from_derive_attributes() {
    let tool = SearchFilesTool;
    let meta = tool.metadata();
    assert_eq!(meta.category, ToolCategory::Search);
    assert_eq!(meta.tags, vec!["file", "search", "pattern"]);
    assert!(matches!(meta.cost_hint, tools_core::CostHint::Free));
}

#[test]
fn test_default_metadata_when_no_attributes() {
    let tool = NoopTool;
    let meta = tool.metadata();
    assert_eq!(meta.category, ToolCategory::General);
    assert!(meta.tags.is_empty());
}

// ── Exposure policy ─────────────────────────────────────────

#[test]
fn test_default_exposure_policy_is_all_false_forbidden() {
    let tool = NoopTool;
    let policy = tool.exposure_policy();
    assert_eq!(policy, ExposurePolicy::default());
    assert_eq!(policy.llm_channels, ChannelMask::ALL);
    assert!(!policy.subagent);
    assert_eq!(policy.mcp, McpExposure::Forbidden);
    // Thin accessors match policy
    assert_eq!(tool.allowed_channels(), policy.llm_channels);
    assert_eq!(tool.subagent_visible(), policy.subagent);
}

#[derive(tools_core::Tool)]
#[tool(
    name = "exposed_echo",
    description = "Echo with MCP Default",
    params = "EchoParams",
    mcp_exposure = "default",
    subagent = "true",
    allowed_channels = "desktop_only"
)]
pub struct ExposedEchoTool;

#[async_trait]
impl ToolExecute for ExposedEchoTool {
    type Params = EchoParams;
    type Ctx<'a> = ();

    async fn execute<'c>(&self, params: EchoParams, _ctx: ()) -> common::Result<String> {
        Ok(params.text)
    }
}

#[test]
fn test_derive_exposure_policy_overrides() {
    let tool = ExposedEchoTool;
    let policy = tool.exposure_policy();
    assert_eq!(policy.llm_channels, ChannelMask::DESKTOP_ONLY);
    assert!(policy.subagent);
    assert_eq!(policy.mcp, McpExposure::Default);
    assert_eq!(tool.allowed_channels(), ChannelMask::DESKTOP_ONLY);
    assert!(tool.subagent_visible());
}

#[derive(tools_core::Tool)]
#[tool(
    name = "opt_in_tool",
    description = "MCP OptIn only",
    params = "NoParams",
    mcp_exposure = "opt_in"
)]
pub struct OptInTool;

#[async_trait]
impl ToolExecute for OptInTool {
    type Params = NoParams;
    type Ctx<'a> = ();

    async fn execute<'c>(&self, _params: NoParams, _ctx: ()) -> common::Result<String> {
        Ok("ok".into())
    }
}

#[test]
fn test_derive_mcp_opt_in_keeps_channel_and_subagent_defaults() {
    let tool = OptInTool;
    let policy = tool.exposure_policy();
    assert_eq!(policy.mcp, McpExposure::OptIn);
    assert_eq!(policy.llm_channels, ChannelMask::ALL);
    assert!(!policy.subagent);
}

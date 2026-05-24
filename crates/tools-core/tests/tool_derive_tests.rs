//! Tests for `#[derive(Tool)]` macro — generates Tool trait from ToolExecute + metadata.

use async_trait::async_trait;
use common::{ChannelName, ChatId};
use serde_json::json;
use tools_core::{RoutingContext, Tool as ToolTrait, ToolCategory, ToolExecute, ToolParams};

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

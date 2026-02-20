//! Integration tests for `#[tool_actions]` attribute macro (AC 2.4).

use common::{ChannelName, ChatId};
use serde_json::json;
use tools_core::{tool_actions, ActionParams, RoutingContext, Tool};

// --- Param structs ---

#[derive(Debug, ActionParams)]
pub struct GreetParams {
    /// Name to greet
    #[param(required)]
    pub name: String,
}

#[derive(Debug, ActionParams)]
pub struct FarewellParams {
    /// Name to bid farewell
    #[param(required)]
    pub name: String,
    /// Include emoji
    pub emoji: Option<bool>,
}

// --- Test tool ---

pub struct TestTool;

#[tool_actions(name = "test_tool", description = "A test tool")]
impl TestTool {
    #[action(name = "greet")]
    async fn handle_greet(
        &self,
        params: GreetParams,
        _ctx: &RoutingContext,
    ) -> common::Result<String> {
        Ok(format!("Hello, {}!", params.name))
    }

    #[action(name = "farewell")]
    async fn handle_farewell(
        &self,
        params: FarewellParams,
        _ctx: &RoutingContext,
    ) -> common::Result<String> {
        Ok(format!("Goodbye, {}!", params.name))
    }
}

fn ctx() -> RoutingContext {
    RoutingContext::new(ChannelName::from("test"), ChatId::from("123"))
}

// ============================================================
// AC 2.4: Generated name matches attribute
// ============================================================

#[test]
fn test_generated_name_matches_attribute() {
    let tool = TestTool;
    assert_eq!(tool.name(), "test_tool");
}

// ============================================================
// AC 2.4: Generated description matches attribute
// ============================================================

#[test]
fn test_generated_description_matches_attribute() {
    let tool = TestTool;
    assert_eq!(tool.description(), "A test tool");
}

// ============================================================
// AC 2.4: Parameters has action enum
// ============================================================

#[test]
fn test_parameters_has_action_enum() {
    let tool = TestTool;
    let params = tool.parameters();
    let action_enum = params["properties"]["action"]["enum"]
        .as_array()
        .unwrap();
    assert!(action_enum.iter().any(|v| v == "greet"));
    assert!(action_enum.iter().any(|v| v == "farewell"));

    // "action" should be in required
    let required = params["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "action"));
}

#[test]
fn test_parameters_has_merged_properties() {
    let tool = TestTool;
    let params = tool.parameters();
    // "name" comes from both GreetParams and FarewellParams
    assert!(params["properties"]["name"].is_object());
    // "emoji" comes from FarewellParams only
    assert!(params["properties"]["emoji"].is_object());
}

// ============================================================
// AC 2.4: Dispatch to correct action handler
// ============================================================

#[tokio::test]
async fn test_dispatch_to_greet_action() {
    let tool = TestTool;
    let result = tool
        .execute(json!({"action": "greet", "name": "World"}), &ctx())
        .await
        .unwrap();
    assert_eq!(result, "Hello, World!");
}

#[tokio::test]
async fn test_dispatch_to_farewell_action() {
    let tool = TestTool;
    let result = tool
        .execute(
            json!({"action": "farewell", "name": "World"}),
            &ctx(),
        )
        .await
        .unwrap();
    assert_eq!(result, "Goodbye, World!");
}

// ============================================================
// AC 2.4: Unknown action returns error
// ============================================================

#[tokio::test]
async fn test_unknown_action_returns_error() {
    let tool = TestTool;
    let result = tool
        .execute(json!({"action": "dance"}), &ctx())
        .await;
    assert!(result.is_err());
}

// ============================================================
// AC 2.4: Missing action field returns error
// ============================================================

#[tokio::test]
async fn test_missing_action_field_returns_error() {
    let tool = TestTool;
    let result = tool
        .execute(json!({"name": "World"}), &ctx())
        .await;
    assert!(result.is_err());
}

// ============================================================
// AC 2.4: Empty/null action field returns error
// ============================================================

#[tokio::test]
async fn test_null_action_field_returns_error() {
    let tool = TestTool;
    let result = tool
        .execute(json!({"action": null}), &ctx())
        .await;
    assert!(result.is_err());
}

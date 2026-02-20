//! Integration tests for `#[tool_actions]` attribute macro (AC 2.4).
//!
//! tool_actions generates:
//! - `Tool::name()` from attribute
//! - `Tool::description()` from attribute
//! - `Tool::parameters()` with action enum and merged schemas
//! - `Tool::execute()` dispatching to correct handler by action name
//!
//! These tests live in tools-core (not tools-core-macros) because they need
//! the `Tool` trait, `RoutingContext`, and `async_trait`.
//!
//! Dev: implement these tests via TDD alongside the macro in
//! `crates/tools-core-macros/src/tool_actions.rs` and the Tool trait in
//! `crates/tools-core/src/lib.rs`.

// NOTE: The exact import paths will depend on how tools-core re-exports the macros.
// Adjust imports once tools-core/src/lib.rs is finalized.
//
// Expected imports:
//   use tools_core::{ActionParams, tool_actions, Tool, RoutingContext};
//   use common::{ChannelName, ChatId, Result};
//   use serde_json::{json, Value};

// --- Test tool: two actions with different param structs ---

// #[derive(ActionParams)]
// pub struct GreetParams {
//     /// Name to greet
//     #[param(required)]
//     pub name: String,
// }
//
// #[derive(ActionParams)]
// pub struct FarewellParams {
//     /// Name to bid farewell
//     #[param(required)]
//     pub name: String,
//     /// Include emoji
//     pub emoji: Option<bool>,
// }
//
// pub struct TestTool;
//
// #[tool_actions(name = "test_tool", description = "A test tool")]
// impl TestTool {
//     #[action(name = "greet")]
//     async fn handle_greet(&self, params: GreetParams, _ctx: &RoutingContext) -> Result<String> {
//         Ok(format!("Hello, {}!", params.name))
//     }
//
//     #[action(name = "farewell")]
//     async fn handle_farewell(&self, params: FarewellParams, _ctx: &RoutingContext) -> Result<String> {
//         Ok(format!("Goodbye, {}!", params.name))
//     }
// }

// ============================================================
// AC 2.4: Generated name matches attribute
// ============================================================

#[test]
fn test_generated_name_matches_attribute() {
    // Verifies: TestTool.name() == "test_tool".
    // AC 2.4
    todo!()
}

// ============================================================
// AC 2.4: Generated description matches attribute
// ============================================================

#[test]
fn test_generated_description_matches_attribute() {
    // Verifies: TestTool.description() == "A test tool".
    // AC 2.4
    todo!()
}

// ============================================================
// AC 2.4: Parameters has action enum
// ============================================================

#[test]
fn test_parameters_has_action_enum() {
    // Verifies: tool.parameters()["properties"]["action"]["enum"] contains ["greet", "farewell"].
    // Also verifies: "action" is in the "required" array.
    // AC 2.4
    todo!()
}

#[test]
fn test_parameters_has_merged_properties() {
    // Verifies: tool.parameters()["properties"] includes "name" (from both actions)
    //           and "emoji" (from farewell only).
    // AC 2.4
    todo!()
}

// ============================================================
// AC 2.4: Dispatch to correct action handler
// ============================================================

// #[tokio::test]
#[test]
fn test_dispatch_to_greet_action() {
    // Verifies: tool.execute({"action": "greet", "name": "World"}, &ctx).await
    //           == Ok("Hello, World!").
    // AC 2.4
    todo!()
}

// #[tokio::test]
#[test]
fn test_dispatch_to_farewell_action() {
    // Verifies: tool.execute({"action": "farewell", "name": "World", "emoji": true}, &ctx).await
    //           == Ok("Goodbye, World!").
    // AC 2.4
    todo!()
}

// ============================================================
// AC 2.4: Unknown action returns error
// ============================================================

// #[tokio::test]
#[test]
fn test_unknown_action_returns_error() {
    // Verifies: tool.execute({"action": "dance"}, &ctx).await.is_err().
    // AC 2.4
    todo!()
}

// ============================================================
// AC 2.4: Missing action field returns error
// ============================================================

// #[tokio::test]
#[test]
fn test_missing_action_field_returns_error() {
    // Verifies: tool.execute({"name": "World"}, &ctx).await.is_err()
    //           (no "action" field provided).
    // AC 2.4
    todo!()
}

// ============================================================
// AC 2.4: Empty/null action field returns error
// ============================================================

// #[tokio::test]
#[test]
fn test_null_action_field_returns_error() {
    // Verifies: tool.execute({"action": null}, &ctx).await.is_err().
    // AC 2.4
    todo!()
}

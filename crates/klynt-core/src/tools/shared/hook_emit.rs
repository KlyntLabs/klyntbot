//! Helpers for klynt-core tools to fire PreToolUse / PostToolUse hooks at
//! their execute() boundaries.

use klynt_hooks::engine::{HookEngine, HookFireInput, HookOutcome};
use std::sync::Arc;

/// Fire PreToolUse and return:
/// - `Ok(None)` if no engine, or hooks said allow.
/// - `Ok(Some(modified))` if a hook returned modify_args (caller should swap args).
/// - `Err(reason)` if a hook returned block.
pub async fn fire_pre_tool_use(
    engine: Option<&Arc<HookEngine>>,
    session_id: String,
    tool: &str,
    args: &impl serde::Serialize,
    _cwd: Option<String>,
) -> Result<Option<serde_json::Value>, String> {
    let Some(e) = engine else { return Ok(None) };
    let args = serde_json::to_value(args).unwrap_or_default();
    let input = klynt_hooks::events::pre_tool_use::PreToolUseInput {
        session_id,
        tool: tool.to_string(),
        args,
        base: Default::default(),
    };
    match e.fire(HookFireInput::PreToolUse(input)).await {
        HookOutcome::Allow | HookOutcome::LifecycleNoOp => Ok(None),
        HookOutcome::Block { reason } => Err(reason),
        HookOutcome::ModifyArgs { args } => Ok(Some(args)),
    }
}

/// Fire PostToolUse. Errors are swallowed (logged) — post-hooks never abort
/// the call.
pub async fn fire_post_tool_use(
    engine: Option<&Arc<HookEngine>>,
    session_id: String,
    tool: &str,
    success: bool,
    duration_ms: u64,
) {
    let Some(e) = engine else { return };
    let input = klynt_hooks::events::post_tool_use::PostToolUseInput {
        session_id,
        tool: tool.to_string(),
        success,
        duration_ms,
        output_summary: None,
        base: Default::default(),
    };
    let _ = e.fire(HookFireInput::PostToolUse(input)).await;
}

/// Fire PreFileEdit.
pub async fn fire_pre_file_edit(
    engine: Option<&Arc<HookEngine>>,
    session_id: String,
    tool: &str,
    path: &str,
    op: &str,
    diff_preview: String,
    bytes_before: u64,
    bytes_after: u64,
) -> Result<Option<serde_json::Value>, String> {
    let Some(e) = engine else { return Ok(None) };
    let input = klynt_hooks::events::pre_file_edit::PreFileEditInput {
        session_id,
        tool: tool.to_string(),
        path: path.to_string(),
        op: op.to_string(),
        diff_preview,
        bytes_before,
        bytes_after,
        base: Default::default(),
    };
    match e.fire(HookFireInput::PreFileEdit(input)).await {
        HookOutcome::Allow | HookOutcome::LifecycleNoOp => Ok(None),
        HookOutcome::Block { reason } => Err(reason),
        HookOutcome::ModifyArgs { args } => Ok(Some(args)),
    }
}

/// Fire PostFileEdit.
pub async fn fire_post_file_edit(
    engine: Option<&Arc<HookEngine>>,
    session_id: String,
    tool: &str,
    path: &str,
    op: &str,
    bytes_delta: i64,
    success: bool,
) {
    let Some(e) = engine else { return };
    let input = klynt_hooks::events::post_file_edit::PostFileEditInput {
        session_id,
        tool: tool.to_string(),
        path: path.to_string(),
        op: op.to_string(),
        bytes_delta,
        success,
        base: Default::default(),
    };
    let _ = e.fire(HookFireInput::PostFileEdit(input)).await;
}

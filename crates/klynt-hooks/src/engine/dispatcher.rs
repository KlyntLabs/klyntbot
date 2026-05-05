//! Per-event dispatch logic for the HookEngine.

use crate::engine::command_runner;
use crate::error::HookResult;
use crate::schema::Hook;
use klynt_protocol::HookEventName;

/// Unified input enum for `HookEngine::fire`.
#[derive(Debug, Clone)]
pub enum HookFireInput {
    PreToolUse(crate::events::pre_tool_use::PreToolUseInput),
    PostToolUse(crate::events::post_tool_use::PostToolUseInput),
    SessionStart(crate::events::session_start::SessionStartInput),
    UserPromptSubmit(crate::events::user_prompt_submit::UserPromptSubmitInput),
    Stop(crate::events::stop::StopInput),
    SessionEnd(crate::events::session_end::SessionEndInput),
    PreCompact(crate::events::pre_compact::PreCompactInput),
    PostCompact(crate::events::post_compact::PostCompactInput),
    PreFileEdit(crate::events::pre_file_edit::PreFileEditInput),
    PostFileEdit(crate::events::post_file_edit::PostFileEditInput),
    Notification(crate::events::notification::NotificationInput),
    SubagentSpawn(crate::events::subagent_spawn::SubagentSpawnInput),
    Error(crate::events::error::ErrorInput),
}

/// Result of firing a hook (or set of hooks) for an event.
#[derive(Debug, Clone)]
pub enum HookOutcome {
    Allow,
    Block { reason: String },
    ModifyArgs { args: serde_json::Value },
    LifecycleNoOp,
}

/// Run a single hook against a serializable input payload.
pub async fn run_hook(
    hook: &Hook,
    input: &impl serde::Serialize,
) -> HookResult<crate::types::HookResponse> {
    let json_input = serde_json::to_string(input)?;
    let result = command_runner::run_command(hook, &json_input).await;

    let stdout = result.stdout;
    let response: crate::types::HookResponse = if stdout.trim().is_empty() {
        crate::types::HookResponse {
            r#continue: true,
            block: false,
            reason: None,
            modify_args: None,
        }
    } else {
        serde_json::from_str(&stdout).unwrap_or(crate::types::HookResponse {
            r#continue: true,
            block: false,
            reason: None,
            modify_args: None,
        })
    };

    Ok(response)
}

/// Dispatch all hooks registered for a given event name.
pub async fn dispatch_event(
    handlers: &std::collections::HashMap<HookEventName, Vec<Hook>>,
    event: HookEventName,
    input: &impl serde::Serialize,
    blockable: bool,
    supports_modify_args: bool,
) -> HookOutcome {
    if let Some(hooks) = handlers.get(&event) {
        for hook in hooks {
            if let Ok(outcome) = run_hook(hook, input).await {
                if blockable && outcome.block {
                    return HookOutcome::Block {
                        reason: outcome.reason.unwrap_or_default(),
                    };
                }
                if supports_modify_args && outcome.modify_args.is_some() {
                    return HookOutcome::ModifyArgs {
                        args: outcome.modify_args.unwrap(),
                    };
                }
            }
        }
    }
    if blockable || supports_modify_args {
        HookOutcome::Allow
    } else {
        HookOutcome::LifecycleNoOp
    }
}

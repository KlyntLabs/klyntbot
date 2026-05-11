use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;
use std::sync::Arc;

#[klynt_command]
pub async fn coding_message_send(
    thread_id: String,
    text: String,
    model: Option<String>,
) -> serde_json::Value {
    let runtime = Arc::clone(&state).coding_runtime();
    let outcome = runtime
        .start_turn(::app_core::runtime::StartTurnRequest {
            thread_id,
            content: text,
            context: None,
            mode: Some("coding".to_string()),
            model,
        })
        .await?;

    let resp = outcome
        .coding_response
        .expect("coding mode returns coding_response");
    Ok(serde_json::to_value(resp).unwrap_or_default())
}

#[klynt_command]
pub async fn coding_turn_interrupt(_thread_id: String, turn_id: String) -> () {
    let runtime = Arc::clone(&state).coding_runtime();
    runtime.cancel_turn(&turn_id).await
}

#[klynt_command]
pub async fn coding_turn_steer(thread_id: String, turn_id: String, text: String) -> () {
    state
        .coding_turn_steer(&thread_id, &turn_id, &text)
        .await
        .map_err(ApiError::from)
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "coding_message_send" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let text = try_field!(dev::get_str(body, "text"));
            let model = body.get("model").and_then(|v| v.as_str()).map(String::from);
            match core.coding_message_send(&thread_id, &text, model).await {
                Ok(resp) => Ok(serde_json::to_value(resp).unwrap_or_default()),
                Err(e) => Err(ApiError::from(e)),
            }
        }
        "coding_turn_interrupt" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let turn_id = try_field!(dev::get_str(body, "turnId"));
            dev::val(
                core.coding_turn_interrupt(&thread_id, &turn_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_turn_steer" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let turn_id = try_field!(dev::get_str(body, "turnId"));
            let text = try_field!(dev::get_str(body, "text"));
            dev::val(
                core.coding_turn_steer(&thread_id, &turn_id, &text)
                    .await
                    .map_err(ApiError::from),
            )
        }
        _ => return None,
    })
}

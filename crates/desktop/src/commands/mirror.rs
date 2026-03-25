use std::sync::Arc;

use cognitive::mirror::{
    FeedbackTarget, MirrorResponse, MirrorState, NarrativeSnippet, RoutingSnapshot, TrendNarrative,
    UserFeedback,
};
use desktop_shared::errors::ApiError;
use tauri::State;
use uuid::Uuid;

use crate::app_core::AppCore;

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "get_mirror_state",
    "get_routing_history",
    "get_mirror_narratives",
    "get_pending_snippets",
    "submit_mirror_feedback",
    "generate_mirror_response",
    "approve_meta_rule",
    "dismiss_meta_rule",
];

#[tauri::command]
pub async fn get_mirror_state(state: State<'_, Arc<AppCore>>) -> Result<MirrorState, ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.get_state().await?)
}

#[tauri::command]
pub async fn get_routing_history(
    state: State<'_, Arc<AppCore>>,
    days: Option<u32>,
) -> Result<Vec<RoutingSnapshot>, ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.get_routing_history(days.unwrap_or(7)).await?)
}

#[tauri::command]
pub async fn get_mirror_narratives(
    state: State<'_, Arc<AppCore>>,
    limit: Option<u32>,
) -> Result<Vec<TrendNarrative>, ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.get_narratives(limit.unwrap_or(10)).await?)
}

#[tauri::command]
pub async fn get_pending_snippets(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<NarrativeSnippet>, ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.get_pending_snippets().await?)
}

#[tauri::command]
pub async fn submit_mirror_feedback(
    state: State<'_, Arc<AppCore>>,
    item_id: Uuid,
    target: FeedbackTarget,
    feedback: UserFeedback,
) -> Result<(), ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.submit_feedback(item_id, target, feedback).await?)
}

#[tauri::command]
pub async fn generate_mirror_response(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<MirrorResponse, ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.generate_mirror_response(query, None).await?)
}

#[tauri::command]
pub async fn approve_meta_rule(
    state: State<'_, Arc<AppCore>>,
    rule_id: Uuid,
) -> Result<(), ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.approve_meta_rule(rule_id).await?)
}

#[tauri::command]
pub async fn dismiss_meta_rule(
    state: State<'_, Arc<AppCore>>,
    rule_id: Uuid,
) -> Result<(), ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.dismiss_meta_rule(rule_id).await?)
}

// ── Dev server dispatch ──────────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "get_mirror_state" => {
            let facade = match core.mirror_facade() {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            dev::val(facade.get_state().await.map_err(ApiError::from))
        }
        "get_routing_history" => {
            let facade = match core.mirror_facade() {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            let days: Option<u32> = dev::get(body, "days");
            dev::val(
                facade
                    .get_routing_history(days.unwrap_or(7))
                    .await
                    .map_err(ApiError::from),
            )
        }
        "get_mirror_narratives" => {
            let facade = match core.mirror_facade() {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            let limit: Option<u32> = dev::get(body, "limit");
            dev::val(
                facade
                    .get_narratives(limit.unwrap_or(10))
                    .await
                    .map_err(ApiError::from),
            )
        }
        "get_pending_snippets" => {
            let facade = match core.mirror_facade() {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            dev::val(facade.get_pending_snippets().await.map_err(ApiError::from))
        }
        "submit_mirror_feedback" => {
            let facade = match core.mirror_facade() {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            let item_id: Uuid = try_field!(dev::get(body, "itemId")
                .or_else(|| dev::get(body, "item_id"))
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: itemId")));
            let target: FeedbackTarget = try_field!(dev::require(body, "target"));
            let feedback: UserFeedback = try_field!(dev::require(body, "feedback"));
            dev::val(
                facade
                    .submit_feedback(item_id, target, feedback)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "generate_mirror_response" => {
            let facade = match core.mirror_facade() {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            let query = try_field!(dev::get_str(body, "query"));
            dev::val(
                facade
                    .generate_mirror_response(query, None)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "approve_meta_rule" => {
            let facade = match core.mirror_facade() {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            // Accept both camelCase (Tauri frontend) and snake_case (API)
            let rule_id: Uuid = try_field!(dev::get(body, "ruleId")
                .or_else(|| dev::get(body, "rule_id"))
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: ruleId")));
            dev::val(
                facade
                    .approve_meta_rule(rule_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "dismiss_meta_rule" => {
            let facade = match core.mirror_facade() {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            // Accept both camelCase (Tauri frontend) and snake_case (API)
            let rule_id: Uuid = try_field!(dev::get(body, "ruleId")
                .or_else(|| dev::get(body, "rule_id"))
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: ruleId")));
            dev::val(
                facade
                    .dismiss_meta_rule(rule_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        _ => return None,
    })
}

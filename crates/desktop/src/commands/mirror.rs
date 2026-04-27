use cognitive::mirror::{
    BrainVersion, FeedbackTarget, MirrorResponse, MirrorState, NarrativeSnippet, RoutingSnapshot,
    TrendNarrative, UserFeedback,
};
use desktop_shared::types::EntityKind;
use desktop_shared::{errors::ApiError, CommandResult};
use desktop_macros::klynt_command;
use uuid::Uuid;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn get_mirror_state() -> MirrorState {
    Ok(state.mirror_facade()?.get_state().await?)
}

#[klynt_command]
pub async fn get_routing_history(
    days: Option<u32>,
) -> Vec<RoutingSnapshot> {
    Ok(state.mirror_facade()?.get_routing_history(days.unwrap_or(7)).await?)
}

#[klynt_command]
pub async fn get_mirror_narratives(
    limit: Option<u32>,
) -> Vec<TrendNarrative> {
    Ok(state.mirror_facade()?.get_narratives(limit.unwrap_or(10)).await?)
}

#[klynt_command]
pub async fn get_pending_snippets() -> Vec<NarrativeSnippet> {
    Ok(state.mirror_facade()?.get_pending_snippets().await?)
}

#[klynt_command]
pub async fn submit_mirror_feedback(
    app: tauri::AppHandle,
    item_id: Uuid,
    target: FeedbackTarget,
    feedback: UserFeedback,
) -> () {
    state.mirror_facade()?.submit_feedback(item_id, target, feedback).await?;
    super::emit_entity_updated(&app, EntityKind::MirrorSnippet, &item_id.to_string());
    Ok(())
}

#[klynt_command]
pub async fn generate_mirror_response(
    query: String,
) -> MirrorResponse {
    Ok(state.mirror_facade()?.generate_mirror_response(query, None).await?)
}

#[klynt_command]
pub async fn approve_meta_rule(
    app: tauri::AppHandle,
    rule_id: Uuid,
) -> () {
    state.mirror_facade()?.approve_meta_rule(rule_id).await?;
    super::emit_entity_updated(&app, EntityKind::MirrorSnippet, &rule_id.to_string());
    Ok(())
}

#[klynt_command]
pub async fn dismiss_meta_rule(
    app: tauri::AppHandle,
    rule_id: Uuid,
) -> () {
    state.mirror_facade()?.dismiss_meta_rule(rule_id).await?;
    super::emit_entity_updated(&app, EntityKind::MirrorSnippet, &rule_id.to_string());
    Ok(())
}

#[klynt_command]
pub async fn get_brain_versions() -> Vec<BrainVersion> {
    Ok(state.mirror_facade()?.get_brain_versions().await?)
}

#[klynt_command]
pub async fn revert_brain_version(
    app: tauri::AppHandle,
    version: u32,
) -> BrainVersion {
    let result = state.mirror_facade()?.revert_to_version(version).await?;
    super::emit_entity_updated(&app, EntityKind::BrainVersion, &version.to_string());
    Ok(result)
}

#[klynt_command]
pub async fn kill_trial(
    app: tauri::AppHandle,
    trial_id: String,
) -> () {
    state.mirror_facade()?.kill_trial(&trial_id).await?;
    super::emit_entity_updated(&app, EntityKind::MirrorSnippet, &trial_id);
    Ok(())
}

#[klynt_command]
pub async fn continue_trial(
    app: tauri::AppHandle,
    trial_id: String,
) -> () {
    state.mirror_facade()?.continue_trial(&trial_id).await?;
    super::emit_entity_updated(&app, EntityKind::MirrorSnippet, &trial_id);
    Ok(())
}

// ── Dev server dispatch ──────────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};

    let facade = match core.mirror_facade() {
        Ok(f) => f,
        Err(e) => return Some(Err(e)),
    };

    Some(match cmd {
        "get_mirror_state" => dev::val(facade.get_state().await.map_err(ApiError::from)),
        "get_routing_history" => {
            let days: Option<u32> = dev::get(body, "days");
            dev::val(
                facade
                    .get_routing_history(days.unwrap_or(7))
                    .await
                    .map_err(ApiError::from),
            )
        }
        "get_mirror_narratives" => {
            let limit: Option<u32> = dev::get(body, "limit");
            dev::val(
                facade
                    .get_narratives(limit.unwrap_or(10))
                    .await
                    .map_err(ApiError::from),
            )
        }
        "get_pending_snippets" => {
            dev::val(facade.get_pending_snippets().await.map_err(ApiError::from))
        }
        "submit_mirror_feedback" => {
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
            let query = try_field!(dev::get_str(body, "query"));
            dev::val(
                facade
                    .generate_mirror_response(query, None)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "approve_meta_rule" => {
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
        "get_brain_versions" => dev::val(facade.get_brain_versions().await.map_err(ApiError::from)),
        "revert_brain_version" => {
            let version: u32 = try_field!(dev::get(body, "version")
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: version")));
            dev::val(
                facade
                    .revert_to_version(version)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "kill_trial" => {
            let trial_id: String = try_field!(dev::get(body, "trialId")
                .or_else(|| dev::get(body, "trial_id"))
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: trialId")));
            dev::val(facade.kill_trial(&trial_id).await.map_err(ApiError::from))
        }
        "continue_trial" => {
            let trial_id: String = try_field!(dev::get(body, "trialId")
                .or_else(|| dev::get(body, "trial_id"))
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: trialId")));
            dev::val(
                facade
                    .continue_trial(&trial_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        _ => return None,
    })
}

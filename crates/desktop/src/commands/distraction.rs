//! Distraction overlay IPC commands.

use std::sync::Arc;

use desktop_shared::commands::LearnedRuleResponse;
use desktop_shared::errors::ApiError;
use feature_productivity::distraction::DistractionInterceptor;
use tauri::State;

use crate::app_core::AppCore;

use super::map_prod_err as map_err;

#[tauri::command]
pub async fn distraction_dismiss(
    state: State<'_, Arc<AppCore>>,
    app_name: String,
) -> Result<(), ApiError> {
    let focus_mgr = state.focus_manager()?;
    focus_mgr
        .record_distraction(&app_name)
        .await
        .map_err(map_err)?;
    Ok(())
}

#[tauri::command]
pub async fn distraction_allow_temp(
    state: State<'_, Arc<AppCore>>,
    pattern: String,
) -> Result<(), ApiError> {
    let interceptor = state.distraction_interceptor()?;
    let mut guard = interceptor.lock().await;
    guard.grant_temp_pass(&pattern);
    Ok(())
}

#[tauri::command]
pub async fn distraction_allow_session(
    state: State<'_, Arc<AppCore>>,
    app_name: String,
    window_title: Option<String>,
    classification: String,
) -> Result<(), ApiError> {
    let (key, pattern_type) =
        DistractionInterceptor::make_key(&app_name, window_title.as_deref());

    let interceptor = state.distraction_interceptor()?;
    let mut guard = interceptor.lock().await;
    guard.whitelist_for_session(&key);
    drop(guard);

    let repos = state.productivity_repos()?;
    repos
        .learned_rules
        .upsert_or_hit(&key, pattern_type, &classification)
        .await
        .map_err(map_err)?;

    Ok(())
}

#[tauri::command]
pub async fn distraction_learned_rules(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<LearnedRuleResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let rules = repos.learned_rules.list_all().await.map_err(map_err)?;
    Ok(rules
        .into_iter()
        .map(|r| LearnedRuleResponse {
            id: r.id.unwrap_or(0),
            pattern: r.pattern,
            pattern_type: r.pattern_type,
            classification: r.classification,
            confidence: r.confidence,
            hit_count: r.hit_count,
            last_used_at: r.last_used_at.to_rfc3339(),
            created_at: r.created_at.to_rfc3339(),
        })
        .collect())
}

#[tauri::command]
pub async fn distraction_delete_rule(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.learned_rules.delete(id).await.map_err(map_err)?;
    Ok(())
}

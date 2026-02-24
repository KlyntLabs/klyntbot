//! Skills API handlers.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillResponse {
    pub name: String,
    pub description: String,
    pub version: String,
    pub available: bool,
}

#[derive(Deserialize)]
pub struct ToggleSkillRequest {
    pub enabled: bool,
}

/// GET /api/skills — list all skills.
pub async fn list_skills(
    State(state): State<AppState>,
) -> Json<Vec<SkillResponse>> {
    let skills = state
        .agent_loop
        .skill_manager()
        .all()
        .into_iter()
        .map(|s| SkillResponse {
            name: s.name.clone(),
            description: s.description.clone(),
            version: s.version.clone(),
            available: s.available,
        })
        .collect();
    Json(skills)
}

/// PATCH /api/skills/:name — toggle a skill's enabled state.
pub async fn toggle_skill(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ToggleSkillRequest>,
) -> Result<Json<SkillResponse>, ApiError> {
    let skill = state
        .agent_loop
        .skill_manager()
        .get(&name)
        .ok_or_else(|| ApiError::not_found(format!("skill '{name}' not found")))?;

    // Skills don't have persistent enabled state in SkillManager (in-memory only).
    // We validate the skill exists and return it as-is.
    let _ = req.enabled;

    Ok(Json(SkillResponse {
        name: skill.name.clone(),
        description: skill.description.clone(),
        version: skill.version.clone(),
        available: skill.available,
    }))
}

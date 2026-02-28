//! Skills API handlers.

use axum::extract::{Path, State};
use axum::http::StatusCode;
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
    pub always: bool,
    pub source: String,
    pub triggers: Vec<String>,
    pub requires_bins: Vec<String>,
    pub requires_env: Vec<String>,
    pub content: Option<String>,
    pub enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSkillRequest {
    pub enabled: Option<bool>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub triggers: Option<Vec<String>>,
    pub always: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkillRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub always: bool,
}

fn default_version() -> String {
    "1.0".to_string()
}

/// Check whether a skill is a built-in (bundled) skill vs. a workspace skill.
fn is_builtin(skill: &agent::skills::Skill) -> bool {
    skill.path.to_string_lossy().starts_with("builtin::")
}

/// Derive source label from the skill's origin.
fn skill_source(skill: &agent::skills::Skill) -> String {
    if is_builtin(skill) {
        "built-in".to_string()
    } else {
        "workspace".to_string()
    }
}

/// Build a SkillResponse from an internal Skill + enabled state.
fn to_response(skill: &agent::skills::Skill, enabled: bool) -> SkillResponse {
    SkillResponse {
        name: skill.name.clone(),
        description: skill.description.clone(),
        version: skill.version.clone(),
        available: skill.available,
        always: skill.always,
        source: skill_source(skill),
        triggers: skill.triggers.clone(),
        requires_bins: skill.requires_bins.clone(),
        requires_env: skill.requires_env.clone(),
        content: skill.content.clone(),
        enabled,
    }
}

/// GET /api/skills — list all skills with enabled state from config.
pub async fn list_skills(State(state): State<AppState>) -> Json<Vec<SkillResponse>> {
    let enabled_set: std::collections::HashSet<String> = {
        let config = state.config.read().unwrap_or_else(|e| e.into_inner());
        config.packs.enabled_skills.iter().cloned().collect()
    };
    let skills = state
        .agent_loop
        .skill_manager()
        .all()
        .into_iter()
        .map(|s| {
            let enabled = if is_builtin(s) {
                enabled_set.contains(&s.name)
            } else {
                true // workspace skills are always enabled
            };
            to_response(s, enabled)
        })
        .collect();
    Json(skills)
}

/// GET /api/skills/:name — get a single skill with full content.
pub async fn get_skill(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<SkillResponse>, ApiError> {
    let skill = state
        .agent_loop
        .skill_manager()
        .get(&name)
        .ok_or_else(|| ApiError::not_found(format!("skill '{name}' not found")))?;

    let enabled = if is_builtin(skill) {
        let config = state.config.read().unwrap_or_else(|e| e.into_inner());
        config.packs.enabled_skills.contains(&skill.name)
    } else {
        true
    };

    Ok(Json(to_response(skill, enabled)))
}

/// Build SKILL.md file content from metadata + body.
fn build_skill_md(
    name: &str,
    description: &str,
    version: &str,
    triggers: &[String],
    always: bool,
    body: &str,
) -> Result<String, ApiError> {
    let mut frontmatter =
        format!("---\nname: {name}\ndescription: {description}\nversion: {version}\n",);

    if !triggers.is_empty() || always {
        let mut meta = serde_json::Map::new();
        let mut klyntbot = serde_json::Map::new();
        if always {
            klyntbot.insert("always".to_string(), serde_json::json!(true));
        }
        if !triggers.is_empty() {
            klyntbot.insert("triggers".to_string(), serde_json::json!(triggers));
        }
        meta.insert("klyntbot".to_string(), serde_json::Value::Object(klyntbot));
        let meta_str = serde_json::to_string(&serde_json::Value::Object(meta))
            .map_err(|e| ApiError::internal(e.to_string()))?;
        frontmatter.push_str(&format!("metadata: '{meta_str}'\n"));
    }

    frontmatter.push_str("---\n\n");
    Ok(format!("{frontmatter}{body}"))
}

/// PATCH /api/skills/:name — toggle enabled state or update workspace skill content.
///
/// For built-in skills: only `enabled` field is respected (toggle via config.packs.enabledSkills).
/// For workspace skills: `description`, `content`, `triggers`, `always` update the SKILL.md on disk.
pub async fn update_skill(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<UpdateSkillRequest>,
) -> Result<Json<SkillResponse>, ApiError> {
    let skill = state
        .agent_loop
        .skill_manager()
        .get(&name)
        .ok_or_else(|| ApiError::not_found(format!("skill '{name}' not found")))?;

    let builtin = is_builtin(skill);

    // Handle enabled toggle for built-in skills
    let mut final_enabled = if builtin {
        let config = state.config.read().unwrap_or_else(|e| e.into_inner());
        config.packs.enabled_skills.contains(&skill.name)
    } else {
        true
    };

    if let Some(enabled) = req.enabled {
        if builtin {
            // Update config.packs.enabledSkills using in-memory config
            let current = {
                state
                    .config
                    .read()
                    .map_err(|e| ApiError::internal(e.to_string()))?
                    .clone()
            };

            let mut config_value =
                serde_json::to_value(&current).map_err(|e| ApiError::internal(e.to_string()))?;

            {
                let packs = config_value
                    .as_object_mut()
                    .and_then(|m| m.get_mut("packs"))
                    .and_then(|p| p.as_object_mut())
                    .ok_or_else(|| ApiError::internal("config.packs is not an object"))?;

                let enabled_list = packs
                    .entry("enabledSkills")
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .ok_or_else(|| ApiError::internal("enabledSkills is not an array"))?;

                let name_val = serde_json::Value::String(name.clone());

                if enabled {
                    if !enabled_list.contains(&name_val) {
                        enabled_list.push(name_val);
                    }
                } else {
                    enabled_list.retain(|v| v != &name_val);
                }
            }

            let updated_config: config::Config = serde_json::from_value(config_value)
                .map_err(|e| ApiError::unprocessable(e.to_string()))?;

            config::save(&updated_config)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;

            {
                let mut config = state
                    .config
                    .write()
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                *config = updated_config;
            }

            final_enabled = enabled;
        }
    }

    // Handle content/metadata updates for workspace skills
    let has_content_update = req.description.is_some()
        || req.content.is_some()
        || req.triggers.is_some()
        || req.always.is_some();

    if has_content_update {
        if builtin {
            return Err(ApiError::unprocessable(
                "cannot update content of a built-in skill",
            ));
        }

        // Merge with existing values
        let description = req.description.unwrap_or_else(|| skill.description.clone());
        let content_body = req
            .content
            .unwrap_or_else(|| skill.content.clone().unwrap_or_default());
        let triggers = req.triggers.unwrap_or_else(|| skill.triggers.clone());
        let always = req.always.unwrap_or(skill.always);

        let file_content = build_skill_md(
            &skill.name,
            &description,
            &skill.version,
            &triggers,
            always,
            &content_body,
        )?;

        // Write to the existing SKILL.md path
        tokio::fs::write(&skill.path, &file_content)
            .await
            .map_err(|e| ApiError::internal(format!("failed to write SKILL.md: {e}")))?;

        // Return updated response with new values
        let response = SkillResponse {
            name: skill.name.clone(),
            description,
            version: skill.version.clone(),
            available: skill.available,
            always,
            source: skill_source(skill),
            triggers,
            requires_bins: skill.requires_bins.clone(),
            requires_env: skill.requires_env.clone(),
            content: Some(content_body),
            enabled: true, // workspace skills are always enabled
        };

        return Ok(Json(response));
    }

    Ok(Json(to_response(skill, final_enabled)))
}

/// POST /api/skills — create a workspace skill (writes SKILL.md to disk).
pub async fn create_skill(
    State(state): State<AppState>,
    Json(req): Json<CreateSkillRequest>,
) -> Result<(StatusCode, Json<SkillResponse>), ApiError> {
    // Validate name
    if req.name.is_empty() {
        return Err(ApiError::unprocessable("skill name is required"));
    }
    if req.name.contains('/') || req.name.contains('\\') || req.name.contains("..") {
        return Err(ApiError::unprocessable(
            "skill name must not contain path separators",
        ));
    }

    // Check it doesn't conflict with an existing skill
    if state.agent_loop.skill_manager().get(&req.name).is_some() {
        return Err(ApiError::unprocessable(format!(
            "skill '{}' already exists",
            req.name
        )));
    }

    // Resolve workspace skills directory
    let skills_dir = {
        let config = state
            .config
            .read()
            .map_err(|e| ApiError::internal(e.to_string()))?;
        config.workspace_path().join("skills")
    };

    let skill_dir = skills_dir.join(&req.name);
    tokio::fs::create_dir_all(&skill_dir)
        .await
        .map_err(|e| ApiError::internal(format!("failed to create skill directory: {e}")))?;

    let body = if req.content.is_empty() {
        format!("# {}\n\nYour skill instructions here.\n", req.name)
    } else {
        req.content.clone()
    };

    let file_content = build_skill_md(
        &req.name,
        &req.description,
        &req.version,
        &req.triggers,
        req.always,
        &body,
    )?;
    let skill_file = skill_dir.join("SKILL.md");

    tokio::fs::write(&skill_file, &file_content)
        .await
        .map_err(|e| ApiError::internal(format!("failed to write SKILL.md: {e}")))?;

    // Return a response (the skill isn't loaded into the manager until restart,
    // but we return the data we wrote so the UI can show it immediately)
    let response = SkillResponse {
        name: req.name,
        description: req.description,
        version: req.version,
        available: true,
        always: req.always,
        source: "workspace".to_string(),
        triggers: req.triggers,
        requires_bins: vec![],
        requires_env: vec![],
        content: Some(body),
        enabled: true,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// DELETE /api/skills/:name — delete a workspace skill from disk.
pub async fn delete_skill(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    // Check skill exists
    let skill = state
        .agent_loop
        .skill_manager()
        .get(&name)
        .ok_or_else(|| ApiError::not_found(format!("skill '{name}' not found")))?;

    // Only workspace skills can be deleted
    if is_builtin(skill) {
        return Err(ApiError::unprocessable("cannot delete a built-in skill"));
    }

    // Delete the skill directory
    let skill_dir = skill
        .path
        .parent()
        .ok_or_else(|| ApiError::internal("skill path has no parent directory"))?;

    tokio::fs::remove_dir_all(skill_dir)
        .await
        .map_err(|e| ApiError::internal(format!("failed to delete skill: {e}")))?;

    Ok(StatusCode::NO_CONTENT)
}

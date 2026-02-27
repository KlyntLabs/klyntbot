//! Settings API handlers — read and patch config sections.

use axum::extract::{Path, State};
use axum::Json;
use serde_json::Value;

use crate::error::ApiError;
use crate::state::AppState;

const SECRET_FIELDS: &[&str] = &[
    "apiKey",
    "token",
    "botToken",
    "appToken",
    "imapPassword",
    "smtpPassword",
    "secret",
    "appSecret",
    "encryptKey",
    "clientSecret",
];

fn redact_secrets(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if SECRET_FIELDS.contains(&key.as_str()) {
                    *val = serde_json::json!("••••••");
                } else {
                    redact_secrets(val);
                }
            }
        }
        Value::Array(arr) => {
            for val in arr.iter_mut() {
                redact_secrets(val);
            }
        }
        _ => {}
    }
}

fn remove_placeholder_values(patch: &mut Value) {
    if let Some(map) = patch.as_object_mut() {
        map.retain(|_, v| v.as_str() != Some("••••••"));
        for val in map.values_mut() {
            remove_placeholder_values(val);
        }
    }
}

fn json_merge_patch(target: &mut Value, patch: Value) {
    match patch {
        Value::Object(patch_map) => {
            if !target.is_object() {
                *target = Value::Object(Default::default());
            }
            let target_map = target.as_object_mut().unwrap();
            for (key, value) in patch_map {
                if value.is_null() {
                    target_map.remove(&key);
                } else {
                    json_merge_patch(target_map.entry(key).or_insert(Value::Null), value);
                }
            }
        }
        other => *target = other,
    }
}

/// GET /api/settings — return full redacted config.
pub async fn get_settings(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let config = state
        .config
        .read()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut v = serde_json::to_value(&*config).map_err(|e| ApiError::internal(e.to_string()))?;
    redact_secrets(&mut v);
    Ok(Json(v))
}

/// GET /api/settings/:section — return a single config section (redacted).
pub async fn get_settings_section(
    State(state): State<AppState>,
    Path(section): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let config = state
        .config
        .read()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let full = serde_json::to_value(&*config).map_err(|e| ApiError::internal(e.to_string()))?;
    let map = full
        .as_object()
        .ok_or_else(|| ApiError::internal("config is not an object"))?;
    let mut section_val = map
        .get(&section)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("settings section '{section}' not found")))?;
    redact_secrets(&mut section_val);
    Ok(Json(section_val))
}

/// PATCH /api/settings/:section — merge-patch a config section and save to disk.
pub async fn patch_settings_section(
    State(state): State<AppState>,
    Path(section): Path<String>,
    Json(mut patch): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    // Reject patches that contain dataDir
    if let Some(obj) = patch.as_object() {
        if obj.contains_key("dataDir") {
            return Err(ApiError::unprocessable(
                "dataDir cannot be changed via the API".to_string(),
            ));
        }
    }

    // Strip placeholder redacted values so we don't write "••••••" to disk
    remove_placeholder_values(&mut patch);

    // Load current config from disk
    let current = config::load()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut config_value =
        serde_json::to_value(&current).map_err(|e| ApiError::internal(e.to_string()))?;

    {
        let config_map = config_value
            .as_object_mut()
            .ok_or_else(|| ApiError::internal("config is not an object"))?;

        // Ensure section exists
        if !config_map.contains_key(&section) {
            return Err(ApiError::not_found(format!(
                "settings section '{section}' not found"
            )));
        }

        // Apply JSON merge-patch (RFC 7396) to the section
        let section_val = config_map.entry(section.clone()).or_insert(Value::Null);
        json_merge_patch(section_val, patch);
    }

    // Re-deserialize back to Config to validate the merged result
    let updated_config: config::Config =
        serde_json::from_value(config_value).map_err(|e| ApiError::unprocessable(e.to_string()))?;

    // Save to disk
    config::save(&updated_config)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    // Update in-memory config so subsequent GET requests reflect the change
    {
        let mut config = state
            .config
            .write()
            .map_err(|e| ApiError::internal(e.to_string()))?;
        *config = updated_config.clone();
    }

    // Return the updated section (redacted)
    let saved_val =
        serde_json::to_value(&updated_config).map_err(|e| ApiError::internal(e.to_string()))?;
    let saved_map = saved_val
        .as_object()
        .ok_or_else(|| ApiError::internal("saved config is not an object"))?;
    let mut result = saved_map.get(&section).cloned().unwrap_or(Value::Null);
    redact_secrets(&mut result);
    Ok(Json(result))
}

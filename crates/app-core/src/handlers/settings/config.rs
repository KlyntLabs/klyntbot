//! Generic config settings handlers.

use desktop_shared::commands::AppInfoResponse;
use desktop_shared::errors::ApiError;
use serde_json::Value;

use crate::errors::{map_config_save_err, map_serialization_err};
use crate::state::AppCore;

/// Recursively merge `patch` into `base`. Objects merge keys;
/// arrays and scalars replace entirely; explicit `null` removes a key.
fn deep_merge(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                if value.is_null() {
                    base_map.remove(&key);
                } else {
                    let entry = base_map.entry(key).or_insert(Value::Null);
                    deep_merge(entry, value);
                }
            }
        }
        (base, patch) => *base = patch,
    }
}

// ── AppCore methods ───────────────────────────────────────────────────

impl AppCore {
    pub async fn app_info(&self) -> Result<AppInfoResponse, ApiError> {
        let cfg = self.config.read().await;
        Ok(AppInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            data_dir: cfg.data_dir_path().to_string_lossy().to_string(),
            setup_completed: cfg.setup_completed,
        })
    }

    pub async fn config_get_section(&self, section: String) -> Result<Value, ApiError> {
        let cfg = self.config.read().await;
        let full = serde_json::to_value(&*cfg).map_err(map_serialization_err)?;
        match full.get(&section) {
            Some(val) => Ok(val.clone()),
            None => Err(ApiError::new(
                "NOT_FOUND",
                format!("config section '{section}' not found"),
            )),
        }
    }

    pub async fn config_update_section(
        &self,
        section: String,
        patch: Value,
    ) -> Result<Value, ApiError> {
        let mut cfg = self.config.write().await;

        let mut full = serde_json::to_value(&*cfg).map_err(map_serialization_err)?;

        {
            let section_val = full.get_mut(&section).ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("config section '{section}' not found"))
            })?;
            deep_merge(section_val, patch);
        }

        // Extract the merged section before from_value consumes `full`
        let section_result = full.get(&section).cloned().unwrap_or(Value::Null);

        let updated: config::Config = serde_json::from_value(full)
            .map_err(|e| ApiError::new("VALIDATION", format!("invalid config: {e}")))?;

        config::save(&updated).await.map_err(map_config_save_err)?;

        *cfg = updated;

        Ok(section_result)
    }

    pub async fn config_mark_setup_completed(&self) -> Result<(), ApiError> {
        let mut cfg = self.config.write().await;
        cfg.setup_completed = true;
        config::save(&cfg).await.map_err(map_config_save_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deep_merge_objects_recursive() {
        let mut base = json!({"a": {"b": 1, "c": 2}, "d": 3});
        let patch = json!({"a": {"b": 10, "e": 5}});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": {"b": 10, "c": 2, "e": 5}, "d": 3}));
    }

    #[test]
    fn test_deep_merge_scalars_overwrite() {
        let mut base = json!({"x": 1});
        let patch = json!({"x": 99});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"x": 99}));
    }

    #[test]
    fn test_deep_merge_arrays_replace() {
        let mut base = json!({"tags": [1, 2, 3]});
        let patch = json!({"tags": [4, 5]});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"tags": [4, 5]}));
    }

    #[test]
    fn test_deep_merge_null_removes_key() {
        let mut base = json!({"a": 1, "b": 2});
        let patch = json!({"a": null});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"b": 2}));
    }

    #[test]
    fn test_deep_merge_adds_new_keys() {
        let mut base = json!({"a": 1});
        let patch = json!({"b": 2, "c": {"d": 3}});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": 1, "b": 2, "c": {"d": 3}}));
    }

    #[test]
    fn test_deep_merge_nested_null_removes() {
        let mut base = json!({"a": {"b": 1, "c": 2}});
        let patch = json!({"a": {"b": null}});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": {"c": 2}}));
    }

    #[test]
    fn test_deep_merge_empty_patch() {
        let mut base = json!({"a": 1});
        let patch = json!({});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": 1}));
    }

    #[test]
    fn test_deep_merge_replaces_scalar_with_object() {
        let mut base = json!({"a": 1});
        let patch = json!({"a": {"nested": true}});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": {"nested": true}}));
    }

    #[test]
    fn test_deep_merge_replaces_object_with_scalar() {
        let mut base = json!({"a": {"nested": true}});
        let patch = json!({"a": 42});
        deep_merge(&mut base, patch);
        assert_eq!(base, json!({"a": 42}));
    }
}

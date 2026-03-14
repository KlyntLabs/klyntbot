//! manifest.json parsing — stub for Subsystem 2 (Declarative Feature Modules).

use std::path::Path;

use crate::types::SkillManifest;

/// Parse a manifest.json file. Returns None if file doesn't exist.
pub async fn parse_manifest(skill_dir: &Path) -> common::Result<Option<SkillManifest>> {
    let manifest_path = skill_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }

    let content = tokio::fs::read_to_string(&manifest_path)
        .await
        .map_err(|e| common::ConfigError::Invalid(format!("Reading manifest.json: {e}")))?;

    let raw: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| common::ConfigError::Invalid(format!("Parsing manifest.json: {e}")))?;

    Ok(Some(SkillManifest {
        schema_version: raw
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0")
            .to_string(),
        entities: raw
            .get("entities")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        permissions: raw
            .get("permissions")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
    }))
}

//! Multi-source content loader.
//!
//! Loads documentation and skill entries from local filesystem directories.
//! Remote loading is deferred to async refresh (future work).

use super::types::*;
use super::ContentRegistry;
use config::ContentConfig;

/// Load content from all configured sources.
pub fn load_all(config: &ContentConfig) -> common::Result<ContentRegistry> {
    let mut registry = ContentRegistry::empty();

    for source in &config.sources {
        if let Some(path) = &source.path {
            load_local(&mut registry, &source.name, path)?;
        }
        // Remote loading deferred to async refresh
    }

    Ok(registry)
}

/// Load content from a local directory.
///
/// Expects a `docs/manifest.json` file with a `docs` array of DocEntry objects,
/// and optionally a `skills/manifest.json` with a `skills` array.
fn load_local(registry: &mut ContentRegistry, name: &str, path: &str) -> common::Result<()> {
    let base = std::path::PathBuf::from(path);

    // Load docs manifest
    let docs_manifest = base.join("docs/manifest.json");
    if docs_manifest.exists() {
        let content = std::fs::read_to_string(&docs_manifest)?;
        let manifest: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(docs) = manifest.get("docs").and_then(|d| d.as_array()) {
            for doc in docs {
                if let Ok(mut entry) = serde_json::from_value::<DocEntry>(doc.clone()) {
                    if entry.content_source.is_empty() {
                        entry.content_source = name.to_string();
                    }
                    registry.add_doc(entry);
                }
            }
        }
    }

    // Load skills manifest
    let skills_manifest = base.join("skills/manifest.json");
    if skills_manifest.exists() {
        let content = std::fs::read_to_string(&skills_manifest)?;
        let manifest: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(skills) = manifest.get("skills").and_then(|s| s.as_array()) {
            for skill in skills {
                if let Ok(mut entry) = serde_json::from_value::<SkillEntry>(skill.clone()) {
                    if entry.content_source.is_empty() {
                        entry.content_source = name.to_string();
                    }
                    registry.add_skill(entry);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::ContentSourceConfig;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_load_from_local_docs_manifest() {
        let dir = TempDir::new().unwrap();
        let docs_dir = dir.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(
            docs_dir.join("manifest.json"),
            r#"{
                "docs": [
                    {
                        "id": "stripe/api",
                        "name": "Stripe API",
                        "description": "Payment processing API",
                        "source": "community",
                        "tags": ["payment", "api"],
                        "contentSource": "",
                        "languages": []
                    }
                ]
            }"#,
        )
        .unwrap();

        let config = ContentConfig {
            sources: vec![ContentSourceConfig {
                name: "test".into(),
                path: Some(dir.path().to_string_lossy().into()),
                url: None,
            }],
            trust_policy: "official,community".into(),
            refresh_interval_secs: 3600,
            content_dir: dir.path().to_path_buf(),
        };

        let registry = load_all(&config).unwrap();
        assert_eq!(registry.docs().len(), 1);
        assert_eq!(registry.docs()[0].id, "stripe/api");
        assert_eq!(registry.docs()[0].content_source, "test");
    }

    #[test]
    fn test_load_empty_directory() {
        let dir = TempDir::new().unwrap();

        let config = ContentConfig {
            sources: vec![ContentSourceConfig {
                name: "empty".into(),
                path: Some(dir.path().to_string_lossy().into()),
                url: None,
            }],
            content_dir: PathBuf::new(),
            ..Default::default()
        };

        let registry = load_all(&config).unwrap();
        assert!(registry.docs().is_empty());
        assert!(registry.skills().is_empty());
    }

    #[test]
    fn test_load_no_sources() {
        let config = ContentConfig::default();
        let registry = load_all(&config).unwrap();
        assert!(registry.docs().is_empty());
    }
}

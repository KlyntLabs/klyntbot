//! `klyntbot plugin update` — update installed plugin(s) to latest version.

use std::path::Path;

use anyhow::{bail, Context, Result};
use plugin_runtime::manager::PluginManager;
use plugin_runtime::manifest::PluginManifest;

/// Update a specific plugin or all installed plugins.
pub async fn run(id: Option<&str>, plugins_dir: &Path, registry_url: &str) -> Result<()> {
    if !plugins_dir.exists() {
        bail!("No plugins installed.");
    }

    let manifests = PluginManager::scan_manifests(plugins_dir)?;
    if manifests.is_empty() {
        bail!("No plugins installed.");
    }

    // Fetch registry
    let client = reqwest::Client::new();
    let index: serde_json::Value = client
        .get(registry_url)
        .header("User-Agent", "klyntbot")
        .send()
        .await?
        .error_for_status()
        .context("failed to fetch plugin registry")?
        .json()
        .await?;

    let registry_plugins = index["plugins"]
        .as_array()
        .context("registry has no plugins array")?;

    let to_update: Vec<&(PluginManifest, std::path::PathBuf)> = if let Some(target_id) = id {
        manifests
            .iter()
            .filter(|(m, _)| m.id == target_id)
            .collect()
    } else {
        manifests.iter().collect()
    };

    if to_update.is_empty() {
        if let Some(target_id) = id {
            bail!("Plugin '{target_id}' is not installed.");
        }
        bail!("No plugins to update.");
    }

    let mut updated = 0;
    for (manifest, _wasm_path) in &to_update {
        let registry_entry = registry_plugins
            .iter()
            .find(|p| p["id"].as_str() == Some(&manifest.id));

        let Some(entry) = registry_entry else {
            println!("{}: not found in registry, skipping.", manifest.id);
            continue;
        };

        let latest = entry["latest_version"].as_str().unwrap_or("0.0.0");
        if latest == manifest.version {
            println!(
                "{}: already up to date (v{}).",
                manifest.id, manifest.version
            );
            continue;
        }

        println!(
            "{}: updating v{} -> v{latest}...",
            manifest.id, manifest.version
        );

        // Re-install from registry
        let source = format!("{}@{}", manifest.id, latest);
        super::install::run(&source, plugins_dir, registry_url).await?;
        updated += 1;
    }

    if updated == 0 {
        println!("All plugins are up to date.");
    } else {
        println!("{updated} plugin(s) updated.");
    }

    Ok(())
}

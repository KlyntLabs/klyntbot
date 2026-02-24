//! `klyntbot plugin list` — display installed plugins.

use std::path::Path;

use anyhow::Result;
use plugin_runtime::manager::PluginManager;

/// List all installed plugins by scanning the plugins directory.
pub async fn run(plugins_dir: &Path) -> Result<()> {
    if !plugins_dir.exists() {
        println!("No plugins installed (plugins directory does not exist).");
        return Ok(());
    }

    let manifests = PluginManager::scan_manifests(plugins_dir)?;
    if manifests.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    println!("{:<24} {:<10} DESCRIPTION", "ID", "VERSION");
    println!("{}", "-".repeat(60));

    for (manifest, _wasm_path) in &manifests {
        println!(
            "{:<24} {:<10} {}",
            manifest.id, manifest.version, manifest.description
        );
    }

    println!("\n{} plugin(s) installed.", manifests.len());
    Ok(())
}

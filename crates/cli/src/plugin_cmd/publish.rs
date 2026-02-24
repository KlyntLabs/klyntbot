//! `klyntbot plugin publish` — publish a plugin to the registry.

use anyhow::{bail, Result};

/// Guide the user through publishing a plugin.
///
/// Publishing is done via a PR to the klyntbot plugin registry repository.
pub async fn run() -> Result<()> {
    // Check that we're in a plugin project directory
    let manifest_path = std::path::Path::new("klyntbot.plugin.json");
    if !manifest_path.exists() {
        bail!(
            "No klyntbot.plugin.json found in current directory.\n\
             Run this command from your plugin project root."
        );
    }

    let manifest = plugin_runtime::manifest::PluginManifest::from_file(manifest_path)?;

    println!("Publishing {} v{}", manifest.name, manifest.version);
    println!();
    println!("To publish your plugin to the klyntbot registry:");
    println!();
    println!("  1. Push your plugin to a public GitHub repository");
    println!("  2. Create a GitHub release with plugin.wasm and klyntbot.plugin.json as assets");
    println!(
        "  3. Open a PR at https://github.com/klyntbot/plugin-registry adding your plugin to index.json"
    );
    println!();
    println!("Registry entry template:");
    println!();

    let entry = serde_json::json!({
        "id": manifest.id,
        "name": manifest.name,
        "description": manifest.description,
        "author": manifest.author,
        "latest_version": manifest.version,
        "download_url": format!("https://github.com/YOUR_USER/{}/releases/latest/download/plugin.wasm", manifest.id),
    });
    println!("{}", serde_json::to_string_pretty(&entry)?);

    Ok(())
}

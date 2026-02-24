//! `klyntbot plugin install` — install from local path, GitHub, or registry.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use plugin_runtime::manifest::PluginManifest;

/// Run plugin install from a source string.
///
/// Source formats:
/// - `./path/to/plugin.wasm` or `/abs/path.wasm` — local file
/// - `github:user/repo` — latest GitHub release
/// - `plugin-id` or `plugin-id@1.0.0` — registry lookup
pub async fn run(source: &str, plugins_dir: &Path, registry_url: &str) -> Result<()> {
    tokio::fs::create_dir_all(plugins_dir).await?;

    if source.starts_with('.') || source.starts_with('/') {
        install_local(source, plugins_dir).await
    } else if let Some(repo) = source.strip_prefix("github:") {
        install_github(repo, plugins_dir).await
    } else {
        install_registry(source, plugins_dir, registry_url).await
    }
}

/// Install from a local `.wasm` file.
///
/// Expects a sibling `klyntbot.plugin.json` manifest next to the wasm file,
/// or the source can be a directory containing both files.
async fn install_local(source: &str, plugins_dir: &Path) -> Result<()> {
    let source_path = PathBuf::from(source);

    let (wasm_path, manifest_path) = if source_path.is_dir() {
        (
            source_path.join("plugin.wasm"),
            source_path.join("klyntbot.plugin.json"),
        )
    } else {
        let parent = source_path
            .parent()
            .context("cannot determine parent directory")?;
        (source_path.clone(), parent.join("klyntbot.plugin.json"))
    };

    if !wasm_path.exists() {
        bail!("WASM file not found: {}", wasm_path.display());
    }
    if !manifest_path.exists() {
        bail!(
            "Manifest not found: {} (expected klyntbot.plugin.json next to the wasm file)",
            manifest_path.display()
        );
    }

    let manifest = PluginManifest::from_file(&manifest_path)?;
    show_permissions(&manifest);

    let dest = plugins_dir.join(&manifest.id);
    tokio::fs::create_dir_all(&dest).await?;
    tokio::fs::copy(&wasm_path, dest.join("plugin.wasm")).await?;
    tokio::fs::copy(&manifest_path, dest.join("klyntbot.plugin.json")).await?;

    println!(
        "Installed {} v{} ({})",
        manifest.name, manifest.version, manifest.id
    );
    Ok(())
}

/// Install from a GitHub release (`user/repo`).
///
/// Fetches the latest release, downloads `plugin.wasm` and `klyntbot.plugin.json` assets.
async fn install_github(repo: &str, plugins_dir: &Path) -> Result<()> {
    let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = reqwest::Client::new();
    let release: serde_json::Value = client
        .get(&api_url)
        .header("User-Agent", "klyntbot")
        .send()
        .await?
        .error_for_status()
        .context("failed to fetch GitHub release")?
        .json()
        .await?;

    let assets = release["assets"]
        .as_array()
        .context("no assets in release")?;

    let wasm_asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some("plugin.wasm"))
        .context("release has no plugin.wasm asset")?;
    let manifest_asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some("klyntbot.plugin.json"))
        .context("release has no klyntbot.plugin.json asset")?;

    let wasm_url = wasm_asset["browser_download_url"]
        .as_str()
        .context("missing download URL for wasm")?;
    let manifest_url = manifest_asset["browser_download_url"]
        .as_str()
        .context("missing download URL for manifest")?;

    // Download manifest first to get plugin ID
    let manifest_bytes = client
        .get(manifest_url)
        .header("User-Agent", "klyntbot")
        .send()
        .await?
        .bytes()
        .await?;
    let manifest: PluginManifest = serde_json::from_slice(&manifest_bytes)?;
    show_permissions(&manifest);

    let dest = plugins_dir.join(&manifest.id);
    tokio::fs::create_dir_all(&dest).await?;
    tokio::fs::write(dest.join("klyntbot.plugin.json"), &manifest_bytes).await?;

    // Download wasm
    println!("Downloading plugin.wasm from {}...", repo);
    let wasm_bytes = client
        .get(wasm_url)
        .header("User-Agent", "klyntbot")
        .send()
        .await?
        .bytes()
        .await?;
    tokio::fs::write(dest.join("plugin.wasm"), &wasm_bytes).await?;

    println!(
        "Installed {} v{} from github:{repo}",
        manifest.name, manifest.version
    );
    Ok(())
}

/// Install from the plugin registry.
///
/// Supports `plugin-id` or `plugin-id@version`.
async fn install_registry(source: &str, plugins_dir: &Path, registry_url: &str) -> Result<()> {
    let (id, version) = match source.split_once('@') {
        Some((id, ver)) => (id, Some(ver)),
        None => (source, None),
    };

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

    let plugins = index["plugins"]
        .as_array()
        .context("registry has no plugins array")?;

    let entry = plugins
        .iter()
        .find(|p| p["id"].as_str() == Some(id))
        .with_context(|| format!("plugin '{id}' not found in registry"))?;

    let dl_version =
        version.unwrap_or_else(|| entry["latest_version"].as_str().unwrap_or("unknown"));
    let download_url = entry["download_url"]
        .as_str()
        .with_context(|| format!("no download URL for plugin '{id}'"))?;

    // Download the plugin archive/wasm
    println!("Installing {id}@{dl_version} from registry...");
    let wasm_bytes = client
        .get(download_url)
        .header("User-Agent", "klyntbot")
        .send()
        .await?
        .bytes()
        .await?;

    let dest = plugins_dir.join(id);
    tokio::fs::create_dir_all(&dest).await?;
    tokio::fs::write(dest.join("plugin.wasm"), &wasm_bytes).await?;

    // Write a minimal manifest if one wasn't included
    let manifest_path = dest.join("klyntbot.plugin.json");
    if !manifest_path.exists() {
        let manifest_json = serde_json::json!({
            "id": id,
            "name": entry["name"].as_str().unwrap_or(id),
            "version": dl_version,
            "description": entry["description"].as_str().unwrap_or(""),
            "author": entry["author"].as_str().unwrap_or("unknown"),
        });
        tokio::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest_json)?,
        )
        .await?;
    }

    println!("Installed {id} v{dl_version}");
    Ok(())
}

/// Display the permissions a plugin requests.
fn show_permissions(manifest: &PluginManifest) {
    if manifest.permissions.is_empty() {
        return;
    }
    println!("Permissions requested by {}:", manifest.name);
    for perm in &manifest.permissions {
        println!("  - {perm:?}");
    }
}

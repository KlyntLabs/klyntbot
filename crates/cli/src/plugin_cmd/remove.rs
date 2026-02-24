//! `klyntbot plugin remove` — uninstall a plugin by ID.

use std::path::Path;

use anyhow::{bail, Result};

/// Remove an installed plugin by deleting its directory.
pub async fn run(id: &str, plugins_dir: &Path) -> Result<()> {
    let plugin_path = plugins_dir.join(id);
    if !plugin_path.exists() {
        bail!("Plugin '{id}' is not installed.");
    }

    tokio::fs::remove_dir_all(&plugin_path).await?;
    println!("Removed plugin '{id}'.");
    Ok(())
}

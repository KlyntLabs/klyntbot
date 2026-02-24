//! PluginManager: discovers and loads all WASM plugins from disk.

use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::host;
use crate::manifest::PluginManifest;
use crate::plugin_package::PluginPackage;

/// Discovers, validates, and loads WASM plugins from `~/.klyntbot/plugins/`.
pub struct PluginManager {
    packages: Vec<PluginPackage>,
    plugins_dir: PathBuf,
}

impl PluginManager {
    /// Scan the plugins directory for `klyntbot.plugin.json` manifests.
    ///
    /// Each subdirectory is expected to contain:
    /// - `klyntbot.plugin.json` — plugin manifest
    /// - `plugin.wasm` — compiled WASM module
    pub fn scan_manifests(plugins_dir: &Path) -> Vec<(PathBuf, PluginManifest)> {
        let mut manifests = Vec::new();

        let entries = match std::fs::read_dir(plugins_dir) {
            Ok(entries) => entries,
            Err(e) => {
                warn!(path = %plugins_dir.display(), error = %e, "Cannot read plugins directory");
                return manifests;
            }
        };

        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }

            let manifest_path = dir.join("klyntbot.plugin.json");
            if !manifest_path.exists() {
                continue;
            }

            match PluginManifest::from_file(&manifest_path) {
                Ok(manifest) => {
                    info!(plugin_id = %manifest.id, path = %dir.display(), "Discovered plugin");
                    manifests.push((dir, manifest));
                }
                Err(e) => {
                    warn!(
                        path = %manifest_path.display(),
                        error = %e,
                        "Failed to parse plugin manifest"
                    );
                }
            }
        }

        manifests
    }

    /// Load all discovered plugins from disk.
    ///
    /// Creates `PluginPackage` instances for each valid plugin.
    /// Failed loads are logged as warnings but don't prevent other plugins from loading.
    pub fn load_all(
        plugins_dir: &Path,
        pool: sqlx::SqlitePool,
        bus_sender: Option<tokio::sync::mpsc::UnboundedSender<bus::OutboundMessage>>,
    ) -> Self {
        let discovered = Self::scan_manifests(plugins_dir);
        let mut packages = Vec::new();

        for (dir, manifest) in discovered {
            match Self::load_plugin(&dir, &manifest, pool.clone(), bus_sender.clone()) {
                Ok(pkg) => {
                    info!(
                        plugin_id = %manifest.id,
                        tools = manifest.tools.len(),
                        "Plugin loaded successfully"
                    );
                    packages.push(pkg);
                }
                Err(e) => {
                    warn!(
                        plugin_id = %manifest.id,
                        error = %e,
                        "Failed to load plugin, skipping"
                    );
                }
            }
        }

        info!(count = packages.len(), "Plugins loaded");

        PluginManager {
            packages,
            plugins_dir: plugins_dir.to_path_buf(),
        }
    }

    /// Load a single plugin from its directory.
    fn load_plugin(
        plugin_dir: &Path,
        manifest: &PluginManifest,
        pool: sqlx::SqlitePool,
        bus_sender: Option<tokio::sync::mpsc::UnboundedSender<bus::OutboundMessage>>,
    ) -> anyhow::Result<PluginPackage> {
        let wasm_path = plugin_dir.join("plugin.wasm");
        if !wasm_path.exists() {
            anyhow::bail!(
                "plugin.wasm not found in {}",
                plugin_dir.display()
            );
        }

        // Build host functions with permission enforcement
        let host_fns = host::build_host_functions(
            pool,
            manifest.id.clone(),
            manifest.permissions.clone(),
            bus_sender,
        );

        // Create extism manifest pointing to the WASM file
        let wasm = extism::Wasm::File {
            path: wasm_path,
            meta: extism::WasmMetadata::default(),
        };
        let extism_manifest = extism::Manifest::new([wasm]);

        // Build the plugin with WASI support and host functions
        let plugin = extism::PluginBuilder::new(extism_manifest)
            .with_wasi(true)
            .with_functions(host_fns)
            .build()?;

        let mut package = PluginPackage::from_manifest(manifest.clone());
        package.attach_plugin(plugin);

        Ok(package)
    }

    /// Consume the manager and return the loaded packages.
    pub fn into_packages(self) -> Vec<PluginPackage> {
        self.packages
    }

    /// Get the plugins directory path.
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Number of loaded plugins.
    pub fn count(&self) -> usize {
        self.packages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_manifest(dir: &Path, id: &str) {
        let manifest_json = format!(
            r#"{{
                "id": "{id}",
                "name": "{id}",
                "version": "0.1.0",
                "description": "Test plugin",
                "author": "Test"
            }}"#
        );
        fs::write(dir.join("klyntbot.plugin.json"), manifest_json).unwrap();
    }

    #[test]
    fn test_scan_manifests_discovers_plugins() {
        let tmp = TempDir::new().unwrap();
        let plugin_a = tmp.path().join("plugin-a");
        let plugin_b = tmp.path().join("plugin-b");
        fs::create_dir(&plugin_a).unwrap();
        fs::create_dir(&plugin_b).unwrap();

        write_manifest(&plugin_a, "plugin-a");
        write_manifest(&plugin_b, "plugin-b");

        let manifests = PluginManager::scan_manifests(tmp.path());
        assert_eq!(manifests.len(), 2);

        let ids: Vec<&str> = manifests.iter().map(|(_, m)| m.id.as_str()).collect();
        assert!(ids.contains(&"plugin-a"));
        assert!(ids.contains(&"plugin-b"));
    }

    #[test]
    fn test_scan_manifests_skips_invalid() {
        let tmp = TempDir::new().unwrap();
        let valid = tmp.path().join("valid");
        let invalid = tmp.path().join("invalid");
        fs::create_dir(&valid).unwrap();
        fs::create_dir(&invalid).unwrap();

        write_manifest(&valid, "valid");
        fs::write(invalid.join("klyntbot.plugin.json"), "not json").unwrap();

        let manifests = PluginManager::scan_manifests(tmp.path());
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].1.id, "valid");
    }

    #[test]
    fn test_scan_manifests_skips_dirs_without_manifest() {
        let tmp = TempDir::new().unwrap();
        let no_manifest = tmp.path().join("no-manifest");
        fs::create_dir(&no_manifest).unwrap();
        fs::write(no_manifest.join("readme.txt"), "hello").unwrap();

        let manifests = PluginManager::scan_manifests(tmp.path());
        assert!(manifests.is_empty());
    }

    #[test]
    fn test_scan_manifests_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let manifests = PluginManager::scan_manifests(tmp.path());
        assert!(manifests.is_empty());
    }

    #[test]
    fn test_scan_manifests_nonexistent_dir() {
        let manifests = PluginManager::scan_manifests(Path::new("/nonexistent/path"));
        assert!(manifests.is_empty());
    }
}

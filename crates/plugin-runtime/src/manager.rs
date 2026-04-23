//! PluginManager: discovers and loads all WASM plugins from disk.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use config::schema::PluginsConfig;
use tracing::{info, warn};

use crate::host;
use crate::manifest::PluginManifest;
use crate::plugin_package::PluginPackage;

/// Discovers, validates, and loads WASM plugins from `~/.klyntbot/plugins/`.
#[allow(dead_code)]
pub struct PluginManager {
    packages: Vec<PluginPackage>,
    plugins_dir: PathBuf,
}

impl PluginManager {
    /// Scan a directory for plugin subdirectories containing `klyntbot.plugin.json`.
    ///
    /// Returns `(manifest, wasm_path)` pairs for each valid plugin found.
    /// Skips subdirectories that lack a manifest or have an invalid one.
    pub fn scan_manifests(dir: &Path) -> common::Result<Vec<(PluginManifest, PathBuf)>> {
        let mut results = Vec::new();

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                warn!(path = %dir.display(), error = %e, "cannot read plugins directory");
                return Ok(results);
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("klyntbot.plugin.json");
            if !manifest_path.exists() {
                continue;
            }

            match PluginManifest::from_file(&manifest_path) {
                Ok(manifest) => {
                    let wasm_path = path.join("plugin.wasm");
                    results.push((manifest, wasm_path));
                }
                Err(e) => {
                    warn!(
                        path = %manifest_path.display(),
                        error = %e,
                        "skipping plugin with invalid manifest"
                    );
                }
            }
        }

        Ok(results)
    }

    /// Load all discovered plugins from disk.
    ///
    /// Returns early with an empty manager if plugins are disabled or the
    /// directory doesn't exist. Failed loads are logged as warnings but don't
    /// prevent other plugins from loading.
    pub fn load_all(
        plugins_dir: &Path,
        pool: sqlx::SqlitePool,
        config: &PluginsConfig,
        bus_sender: Option<tokio::sync::mpsc::Sender<bus::OutboundMessage>>,
        domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> common::Result<Self> {
        if !config.enabled {
            info!("plugin system disabled, skipping plugin loading");
            return Ok(Self {
                packages: Vec::new(),
                plugins_dir: plugins_dir.to_path_buf(),
            });
        }

        if !plugins_dir.exists() {
            info!(dir = %plugins_dir.display(), "plugins directory does not exist, skipping");
            return Ok(Self {
                packages: Vec::new(),
                plugins_dir: plugins_dir.to_path_buf(),
            });
        }

        let discovered = Self::scan_manifests(plugins_dir)?;
        let mut packages = Vec::new();

        for (manifest, wasm_path) in discovered {
            let plugin_id = manifest.id.clone();
            match Self::load_plugin(
                manifest,
                &wasm_path,
                pool.clone(),
                config,
                bus_sender.clone(),
                domain_event_bus.clone(),
            ) {
                Ok(pkg) => {
                    info!(plugin_id = %plugin_id, "loaded plugin");
                    packages.push(pkg);
                }
                Err(e) => {
                    warn!(plugin_id = %plugin_id, error = %e, "failed to load plugin, skipping");
                }
            }
        }

        info!(count = packages.len(), "plugins loaded");

        Ok(Self {
            packages,
            plugins_dir: plugins_dir.to_path_buf(),
        })
    }

    /// Load a single plugin from its manifest and wasm file path.
    fn load_plugin(
        manifest: PluginManifest,
        wasm_path: &Path,
        pool: sqlx::SqlitePool,
        config: &PluginsConfig,
        bus_sender: Option<tokio::sync::mpsc::Sender<bus::OutboundMessage>>,
        domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> common::Result<PluginPackage> {
        let wasm_bytes = std::fs::read(wasm_path)?;

        let host_fns = host::build_host_functions(
            pool,
            manifest.id.clone(),
            manifest.permissions.clone(),
            bus_sender,
            domain_event_bus,
        );

        // Convert sandbox_memory_mb to wasm pages (1 page = 64KB)
        let memory_pages = (config.sandbox_memory_mb as u64 * 1024 * 1024 / (64 * 1024)) as u32;
        let extism_manifest =
            extism::Manifest::new([extism::Wasm::data(wasm_bytes)]).with_memory_max(memory_pages);

        let plugin = extism::Plugin::new(&extism_manifest, host_fns, true).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "failed to create extism plugin: {}",
                e
            )))
        })?;

        let mut package = PluginPackage::from_manifest(manifest);
        package.attach_plugin(plugin);

        Ok(package)
    }

    /// Borrow the loaded packages.
    pub fn packages(&self) -> &[PluginPackage] {
        &self.packages
    }

    /// Consume the manager and return the loaded packages.
    pub fn into_packages(self) -> Vec<PluginPackage> {
        self.packages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_manifest(dir: &std::path::Path, plugin_id: &str, manifest_json: &str) {
        let plugin_dir = dir.join(plugin_id);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("klyntbot.plugin.json"), manifest_json).unwrap();
    }

    fn minimal_manifest_json(id: &str) -> String {
        format!(
            r#"{{"id":"{}","name":"Test","version":"1.0.0","description":"d","author":"a"}}"#,
            id
        )
    }

    #[test]
    fn test_scan_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let results = PluginManager::scan_manifests(tmp.path()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_discovers_plugins() {
        let tmp = TempDir::new().unwrap();
        write_manifest(tmp.path(), "plugin-a", &minimal_manifest_json("plugin-a"));
        write_manifest(tmp.path(), "plugin-b", &minimal_manifest_json("plugin-b"));
        let results = PluginManager::scan_manifests(tmp.path()).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_scan_skips_dir_without_manifest() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("no-manifest")).unwrap();
        write_manifest(tmp.path(), "good", &minimal_manifest_json("good"));
        let results = PluginManager::scan_manifests(tmp.path()).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_scan_skips_invalid_manifest() {
        let tmp = TempDir::new().unwrap();
        let bad_dir = tmp.path().join("bad");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(bad_dir.join("klyntbot.plugin.json"), "NOT JSON").unwrap();
        write_manifest(tmp.path(), "good", &minimal_manifest_json("good"));
        let results = PluginManager::scan_manifests(tmp.path()).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_load_all_disabled() {
        let tmp = TempDir::new().unwrap();
        let config = PluginsConfig {
            enabled: false,
            ..PluginsConfig::default()
        };
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let mgr =
            PluginManager::load_all(tmp.path(), pool.inner().clone(), &config, None, None).unwrap();
        assert!(mgr.packages().is_empty());
    }

    #[tokio::test]
    async fn test_load_all_nonexistent_dir() {
        let config = PluginsConfig::default();
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let mgr = PluginManager::load_all(
            Path::new("/nonexistent"),
            pool.inner().clone(),
            &config,
            None,
            None,
        )
        .unwrap();
        assert!(mgr.packages().is_empty());
    }
}

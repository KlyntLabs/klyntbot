//! PluginManager: discovers and loads all WASM plugins from disk.
//! TODO: Full implementation in Task 7.

use std::path::PathBuf;

use crate::plugin_package::PluginPackage;

#[allow(dead_code)]
pub struct PluginManager {
    packages: Vec<PluginPackage>,
    plugins_dir: PathBuf,
}

impl PluginManager {
    pub fn into_packages(self) -> Vec<PluginPackage> {
        self.packages
    }
}

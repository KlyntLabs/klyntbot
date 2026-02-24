//! PluginPackage: a loaded WASM plugin as a FeaturePackage.
//! TODO: Full implementation in Task 6.

use std::sync::Arc;

use crate::manifest::PluginManifest;

pub struct PluginPackage {
    manifest: Arc<PluginManifest>,
}

impl PluginPackage {
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}
